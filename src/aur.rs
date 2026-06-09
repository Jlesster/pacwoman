use std::process::Command;
use std::path::PathBuf;
use std::fs;
use std::io::{BufRead, Write};
use serde::Deserialize;
use crate::config::ResolvedConfig;
use crate::render::{info, header, warn};
use crate::callbacks::render_bar;

#[derive(Debug, Deserialize)]
pub struct AurPackage {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Version")]
    pub version: String,
    #[serde(rename = "Description")]
    pub description: Option<String>,
    #[serde(rename = "URL")]
    pub url: Option<String>,
    #[serde(rename = "Maintainer")]
    pub maintainer: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AurSearchResponse {
    pub results: Vec<AurPackage>,
}

pub struct AurClient {
    base_url: String,
}

impl AurClient {
    pub fn new() -> Self {
        Self {
            base_url: "https://aur.archlinux.org/rpc/v5".to_string(),
        }
    }

    fn client(&self) -> reqwest::blocking::Client {
        reqwest::blocking::Client::builder()
            .user_agent("pacwoman/0.1.7")
            .build()
            .unwrap_or_default()
    }

    pub fn search(&self, query: &str) -> Result<Vec<AurPackage>, String> {
        let url = format!("{}/search/{}", self.base_url, query);
        let resp = self.client().get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("AUR API returned status {}", resp.status()));
        }
        let text = resp.text().map_err(|e| e.to_string())?;
        let data: AurSearchResponse = serde_json::from_str(&text).map_err(|e| {
            format!("JSON decode error: {e}\nBody: {}", text)
        })?;
        Ok(data.results)
    }

    pub fn get_info(&self, name: &str) -> Result<AurPackage, String> {
        let url = format!("{}/info/{}", self.base_url, name);
        let resp = self.client().get(url).send().map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("AUR API returned status {}", resp.status()));
        }
        let text = resp.text().map_err(|e| e.to_string())?;
        let data: AurSearchResponse = serde_json::from_str(&text).map_err(|e| {
            format!("JSON decode error: {e}\nBodyS: {}", text)
        })?;
        data.results.into_iter().next().ok_or_else(|| "package not found in AUR".to_string())
    }
}

pub struct BuildManager {
    cache_dir: PathBuf,
    user: String,
}

impl BuildManager {
    pub fn new() -> Result<Self, String> {
        let user = std::env::var("SUDO_USER")
            .or_else(|_| std::env::var("USER"))
            .map_err(|_| "could not determine current user".to_string())?;

        let home = if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            PathBuf::from("/home").join(sudo_user)
        } else {
            std::env::var("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("/root"))
        };

        let cache_dir = home.join(".cache").join("pacwoman").join("aur");

        Ok(Self { cache_dir, user })
    }

    pub fn build_and_install(&self, pkg_name: &str, cfg: &ResolvedConfig) -> Result<PathBuf, String> {
        let build_path = self.cache_dir.join(pkg_name);
        let is_root = unsafe { libc::getuid() } == 0;

        // 1. Ensure the base cache directory is owned by the user.
        if is_root {
            if let Some(base_cache) = self.cache_dir.parent() {
                let mut chown = Command::new("sudo");
                chown.args(["chown", "-R", &format!("{}:{}", self.user, self.user), base_cache.to_str().unwrap()]);
                let _ = chown.status();
            }
        }

        // 2. Create the AUR cache directory.
        if is_root {
            let mut mkdir = Command::new("sudo");
            mkdir.args(["-u", &self.user, "mkdir", "-p", self.cache_dir.to_str().unwrap()]);
            if !mkdir.status().map(|s| s.success()).unwrap_or(false) {
                return Err("failed to create AUR cache directory as user".to_string());
            }
        } else {
            fs::create_dir_all(&self.cache_dir).map_err(|e| format!("failed to create AUR cache directory: {e}"))?;
        }

        // 3. Warm up sudo if not root, so makepkg -s doesn't hang on an invisible prompt
        if !is_root {
            header("Sudo Authentication", cfg);
            info("Requesting sudo access for dependency installation...", cfg);

            let mut sudo_v = Command::new("sudo");
            sudo_v.arg("-v");

            if !sudo_v.status().map(|s| s.success()).unwrap_or(false) {
                return Err("failed to authenticate with sudo; makepkg -s will likely fail".to_string());
            }
            info("Sudo authenticated successfully.", cfg);
        }

        // Helper to execute a command as the user if we are root
        let run_as_user = |args: &[&str], cwd: Option<&PathBuf>| -> std::process::ExitStatus {
            let mut cmd = if is_root {
                let mut sudo = Command::new("sudo");
                sudo.args(["-u", &self.user]);
                sudo
            } else {
                Command::new(args[0])
            };

            let actual_args = if is_root { args } else { &args[1..] };
            cmd.args(actual_args);
            if let Some(path) = cwd {
                cmd.current_dir(path);
            }
            cmd.status().unwrap_or_else(|_| {
                std::process::Command::new("false").status().unwrap()
            })
        };

        // 4. Clone / Update
        if build_path.exists() {
            info(&format!("updating build files for {pkg_name}..."), cfg);
            let status = run_as_user(&["git", "-C", build_path.to_str().unwrap(), "pull"], None);
            if !status.success() {
                if is_root {
                    let mut rm = Command::new("sudo");
                    rm.args(["-u", &self.user, "rm", "-rf", build_path.to_str().unwrap()]);
                    let _ = rm.status();
                } else {
                    fs::remove_dir_all(&build_path).ok();
                }
            }
        }

        if !build_path.exists() {
            info(&format!("cloning {pkg_name} from AUR..."), cfg);
            let clone_url = format!("https://aur.archlinux.org/{}.git", pkg_name);
            let args = ["git", "clone", "--depth", "1", &clone_url, build_path.to_str().unwrap()];
            if !run_as_user(&args, None).success() {
                return Err(format!("failed to clone {pkg_name} from AUR"));
            }
        }

        // 5. Build with makepkg
        header("building package", cfg);

        let home_dir = if let Ok(sudo_user) = std::env::var("SUDO_USER") {
            format!("/home/{}", sudo_user)
        } else {
            std::env::var("HOME").unwrap_or_else(|_| "/root".into())
        };

        let shell_cmd = if is_root {
            format!("sudo -u {} makepkg -s --noconfirm 2>&1", self.user)
        } else {
            "makepkg -s --noconfirm 2>&1".to_string()
        };

        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&shell_cmd)
            .current_dir(&build_path)
            .env("HOME", &home_dir)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| format!("failed to spawn build process: {e}"))?;

        info(&format!("executing makepkg in {}...", build_path.display()), cfg);

        // Print an initial 0% progress bar so the user knows the build has started
        let initial_col = cfg.bar.upgrade_color.resolve(&cfg.colors).to_string();
        let initial_bar = crate::callbacks::render_bar(0.0, &initial_col, cfg);
        print!(
            "\r\x1b[2K  {surf}{sym}{RST}  {mauve}{build}{RST}  {text}{name}{RST}  {bar_s}  {sub}{pct:>3}%{RST}",
            surf = cfg.colors.surface2,
            sym  = cfg.symbols.box_bar,
            mauve = cfg.colors.mauve,
            build = "building",
            text  = cfg.colors.text,
            name  = pkg_name,
            bar_s = initial_bar,
            sub   = cfg.colors.subtext1,
            pct   = 0,
            RST   = cfg.colors.reset,
        );
        let _ = std::io::stdout().flush();

        let reader = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut current_progress: Option<f64> = Some(0.0);

        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if l.starts_with("==>") {
                        if current_progress.is_some() {
                            println!("\r\x1b[2K");
                            current_progress = None;
                        }

                        println!(
                            "  {mauve}{bold}{sym}{reset} {text}{line}{reset}",
                            mauve = cfg.colors.mauve,
                            bold = cfg.colors.bold,
                            sym = cfg.symbols.bullet,
                            text = cfg.colors.text,
                            reset = cfg.colors.reset,
                            line = l
                        );
                    } else if l.contains("Error") || l.contains("FAILED") || l.contains("critical") {
                        if current_progress.is_some() {
                            println!("\r\x1b[2K");
                            current_progress = None;
                        }
                        println!(
                            "    {red}{bold}{sym} {line}{reset}",
                            red = cfg.colors.red,
                            bold = cfg.colors.bold,
                            sym = cfg.symbols.error,
                            line = l,
                            reset = cfg.colors.reset
                        );
                    } else {
                        let mut found_progress = false;
                        let mut pct = 0.0;

                        if let Some(start) = l.find("[%") {
                            if let Some(end) = l[start..].find('%') {
                                if let Ok(p) = l[start + 1..start + end].trim().parse::<f64>() {
                                    pct = p / 100.0;
                                    found_progress = true;
                                }
                            }
                        }

                        if !found_progress {
                            if let Some(start) = l.find('[') {
                                if let Some(end) = l[start..].find(']') {
                                    let part = &l[start + 1..start + end];
                                    let frags: Vec<&str> = part.split('/').collect();
                                    if frags.len() == 2 {
                                        if let (Ok(c), Ok(t)) = (frags[0].trim().parse::<f64>(), frags[1].trim().parse::<f64>()) {
                                            pct = c / t;
                                            found_progress = true;
                                        }
                                    }
                                }
                            }
                        }

                        if found_progress {
                            current_progress = Some(pct);
                            let col = cfg.bar.upgrade_color.resolve(&cfg.colors).to_string();
                            let bar_s = render_bar(pct, &col, cfg);
                            print!(
                                "\r\x1b[2K  {surf}{sym}{RST}  {mauve}{build}{RST}  {text}{name}{RST}  {bar_s}  {sub}{pct:>3.0}%{RST}",
                                surf = cfg.colors.surface2,
                                sym  = cfg.symbols.box_bar,
                                mauve = cfg.colors.mauve,
                                build = "building",
                                text  = cfg.colors.text,
                                name  = pkg_name,
                                bar_s = bar_s,
                                sub   = cfg.colors.subtext1,
                                pct   = pct * 100.0,
                                RST   = cfg.colors.reset,
                            );
                            let _ = std::io::stdout().flush();
                        } else {
                            let l_lower = l.to_lowercase();
                            let interesting = [
                                "configuring", "found", "checking", "patching",
                                "installing", "creating", "updating", "processing",
                            ];
                            if interesting.iter().any(|&k| l_lower.contains(k)) {
                                if current_progress.is_some() {
                                    println!("\r\x1b[2K");
                                    current_progress = None;
                                }
                                println!(
                                    "  {sub}{line}{reset}",
                                    sub = cfg.colors.subtext1,
                                    line = l,
                                    reset = cfg.colors.reset,
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn(&format!("error reading build output: {e}"), cfg);
                }
            }
        }

        let status = child.wait().map_err(|e| format!("failed to wait for build process: {e}"))?;
        if !status.success() {
            return Err(format!("makepkg failed for {pkg_name} (see output above)"));
        }

        let entries = fs::read_dir(&build_path).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().map_or(false, |ext| ext == "zst") {
                if path.file_name().unwrap().to_string_lossy().contains(".pkg.tar") {
                    return Ok(path);
                }
            }
        }

        Err("could not find built package artifact".to_string())
    }
}
