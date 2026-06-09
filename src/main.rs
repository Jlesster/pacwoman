mod callbacks;
mod config;
mod query;
mod render;
mod aur;

use alpm::{Alpm, AlpmListMut, PackageReason, SigLevel, TransFlag};
use ctrlc;
use query::{query, query_owns, query_search, QueryOpts};
use render::*;
use std::io::IsTerminal;
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const ROOT: &str = "/";
const DBPATH: &str = "/var/lib/pacman";
const LOGFILE: &str = "/var/log/pacman.log";
const GPGDIR: &str = "/etc/pacman.d/gnupg";
const CACHEDIRS: &[&str] = &["/var/cache/pacman/pkg/"];

// ── CLI args ──────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Cli {
    op: Op,
    targets: Vec<String>,
    refresh: u8,
    sysupgrade: u8,
    downloadonly: bool,
    nosave: bool,
    recursive: u8,
    cascade: bool,
    unneeded: bool,
    q_info: bool,

    q_deps: bool,
    q_explicit: bool,
    q_unreq: bool,
    q_upgrades: bool,
    q_quiet: bool,
    q_owns: bool,
    q_search: bool,
    q_file_check: bool,
    q_foreign: bool,
    q_native: bool,
    q_list: bool,
    q_groups: bool,
    q_changelog: bool,
    q_file_query: bool,
    s_info: bool,
    s_list: bool,
    s_search: bool,
    s_clean: bool,
    s_groups: bool,
    s_quiet: bool,
    s_aur_only: bool,
    s_sync_only: bool,
    noconfirm: bool,
    version: bool,
    install_local: Option<String>,
    help: bool,
    print_format: Option<String>,
    deptest: bool,
    asdeps: bool,
    asexplicit: bool,
    plain: bool,
    db_check: bool,
    needed: bool,
    noprogressbar: bool,
    config: Option<String>,
    root: Option<String>,
    dbpath: Option<String>,
    cachedirs: Vec<String>,
    gpgdir: Option<String>,
    hookdir: Option<String>,
    logfile: Option<String>,
    ignorepkg: Vec<String>,
    ignoregroup: Vec<String>,
    overwrite: Vec<String>,
    nodeps: u8,
    assume_installed: Vec<String>,
    dbonly: bool,
    noscriptlet: bool,
    color: Option<String>,
    print: bool,
    verbose: u8,
    confirm: bool,
    arch: Option<String>,
}

#[derive(Default, Debug, PartialEq)]
enum Op {
    #[default]
    None,
    Sync,
    Remove,
    Upgrade,
    Query,
    Database,
    Files,
    CheckConfig,
    GenConfig,
    Declarative,
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut cli = Cli::default();
        cli.plain = !std::io::stdout().is_terminal();
        let args: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            if arg.starts_with("--") {
                match arg.as_str() {
                    "--sync" => cli.op = Op::Sync,
                    "--remove" => cli.op = Op::Remove,
                    "--upgrade" => cli.op = Op::Upgrade,
                    "--query" => cli.op = Op::Query,
                    "--database" => cli.op = Op::Database,
                    "--files" => cli.op = Op::Files,
                    "--check-config" => cli.op = Op::CheckConfig,
                    "--gen-config" => cli.op = Op::GenConfig,
                    "--declarative" => cli.op = Op::Declarative,
                    "--refresh" => cli.refresh += 1,
                    "--sysupgrade" => cli.sysupgrade += 1,
                    "--downloadonly" => cli.downloadonly = true,
                    "--nosave" => cli.nosave = true,
                    "--recursive" => cli.recursive += 1,
                    "--cascade" => cli.cascade = true,
                    "--unneeded" => cli.unneeded = true,
                    "--info" => cli.q_info = true,

                    "--deps" => cli.q_deps = true,
                    "--explicit" => cli.q_explicit = true,
                    "--unrequired" => cli.q_unreq = true,
                    "--upgrades" => cli.q_upgrades = true,
                    "--quiet" => cli.q_quiet = true,
                    "--owns" => cli.q_owns = true,
                    "--search" => cli.q_search = true,
                    "--foreign" => cli.q_foreign = true,
                    "--native" => cli.q_native = true,
                    "--list" => cli.q_list = true,
                    "--groups" => cli.q_groups = true,
                    "--changelog" => cli.q_changelog = true,
                    "--file" => cli.q_file_query = true,
                    "--noconfirm" => cli.noconfirm = true,
                    "--asdeps" => cli.asdeps = true,
                    "--asexplicit" => cli.asexplicit = true,
                    "--plain" => cli.plain = true,
                    "--version" => cli.version = true,
                    "--help" => cli.help = true,
                    "--needed" => cli.needed = true,
                    "--noprogressbar" => cli.noprogressbar = true,
                    "--nodeps" => cli.nodeps += 1,
                    "--dbonly" => cli.dbonly = true,
                    "--noscriptlet" => cli.noscriptlet = true,
                    "--aur-only" => cli.s_aur_only = true,
                    "--sync-only" => cli.s_sync_only = true,
                    "--install-local" => {
                        i += 1;
                        if i < args.len() {
                            cli.install_local = Some(args[i].clone());
                        } else {
                            return Err("missing value for --install-local".to_string());
                        }
                    }
                    "--color" => {
                        i += 1;
                        if i < args.len() {
                            cli.color = Some(args[i].clone());
                        } else {
                            return Err("missing value for --color".to_string());
                        }
                    }
                    "--assume-installed" => {
                        i += 1;
                        if i < args.len() {
                            cli.assume_installed.push(args[i].clone());
                        } else {
                            return Err("missing value for --assume-installed".to_string());
                        }
                    }
                    "--config" => {
                        i += 1;
                        if i < args.len() {
                            cli.config = Some(args[i].clone());
                        } else {
                            return Err("missing value for --config".to_string());
                        }
                    }
                    "--root" => {
                        i += 1;
                        if i < args.len() {
                            cli.root = Some(args[i].clone());
                        } else {
                            return Err("missing value for --root".to_string());
                        }
                    }
                    "--dbpath" => {
                        i += 1;
                        if i < args.len() {
                            cli.dbpath = Some(args[i].clone());
                        } else {
                            return Err("missing value for --dbpath".to_string());
                        }
                    }
                    "--cachedir" => {
                        i += 1;
                        if i < args.len() {
                            cli.cachedirs.push(args[i].clone());
                        } else {
                            return Err("missing value for --cachedir".to_string());
                        }
                    }
                    "--gpgdir" => {
                        i += 1;
                        if i < args.len() {
                            cli.gpgdir = Some(args[i].clone());
                        } else {
                            return Err("missing value for --gpgdir".to_string());
                        }
                    }
                    "--hookdir" => {
                        i += 1;
                        if i < args.len() {
                            cli.hookdir = Some(args[i].clone());
                        } else {
                            return Err("missing value for --hookdir".to_string());
                        }
                    }
                    "--logfile" => {
                        i += 1;
                        if i < args.len() {
                            cli.logfile = Some(args[i].clone());
                        } else {
                            return Err("missing value for --logfile".to_string());
                        }
                    }
                    "--ignore" => {
                        i += 1;
                        if i < args.len() {
                            cli.ignorepkg.push(args[i].clone());
                        } else {
                            return Err("missing value for --ignore".to_string());
                        }
                    }
                    "--ignoregroup" => {
                        i += 1;
                        if i < args.len() {
                            cli.ignoregroup.push(args[i].clone());
                        } else {
                            return Err("missing value for --ignoregroup".to_string());
                        }
                    }
                    "--overwrite" => {
                        i += 1;
                        if i < args.len() {
                            cli.overwrite.push(args[i].clone());
                        } else {
                            return Err("missing value for --overwrite".to_string());
                        }
                    }
                    "--print-format" => {
                        i += 1;
                        if i < args.len() {
                            cli.print_format = Some(args[i].clone());
                        } else {
                            return Err("missing value for --print-format".to_string());
                        }
                    }
                    "--deptest" => cli.deptest = true,
                    "--print" => cli.print = true,
                    "--verbose" => cli.verbose += 1,
                    "--confirm" => cli.confirm = true,
                    "--arch" => {
                        i += 1;
                        if i < args.len() {
                            cli.arch = Some(args[i].clone());
                        } else {
                            return Err("missing value for --arch".to_string());
                        }
                    }
                    "--" => {
                        i += 1;
                        while i < args.len() {
                            cli.targets.push(args[i].clone());
                            i += 1;
                        }
                    }
                    _ => return Err(format!("unsupported flag: {arg}")),
                }
            } else if arg.starts_with('-') {
                for c in arg.chars().skip(1) {
                    match c {
                        'S' => cli.op = Op::Sync,
                        'R' => cli.op = Op::Remove,
                        'U' => cli.op = Op::Upgrade,
                        'Q' => cli.op = Op::Query,
                        'D' => cli.op = Op::Database,
                        'F' => cli.op = Op::Files,
                        'y' => cli.refresh += 1,
                        'u' => match cli.op {
                            Op::Query => cli.q_upgrades = true,
                            Op::Remove => cli.unneeded = true,
                            _ => cli.sysupgrade += 1,
                        },
                        'w' => cli.downloadonly = true,
                        'n' => match cli.op {
                            Op::Query => cli.q_native = true,
                            _ => cli.nosave = true,
                        },
                        's' => match cli.op {
                            Op::Remove => cli.recursive += 1,
                            Op::Sync => cli.s_search = true,
                            Op::Query => cli.q_search = true,
                            _ => {}
                        },
                        'i' => match cli.op {
                            Op::Sync => cli.s_info = true,
                            Op::Query => cli.q_info = true,
                            _ => {}
                        },
                        'l' => match cli.op {
                            Op::Sync => cli.s_list = true,
                            Op::Query => cli.q_list = true,
                            _ => {}
                        },
                        'c' => match cli.op {
                            Op::Sync => cli.s_clean = true,
                            Op::Query => cli.q_changelog = true,
                            Op::Remove => cli.cascade = true,
                            _ => {}
                        },
                        'g' => match cli.op {
                            Op::Sync => cli.s_groups = true,
                            Op::Query => cli.q_groups = true,
                            _ => {}
                        },
                        'd' => match cli.op {
                            Op::Query => cli.q_deps = true,
                            Op::Sync | Op::Upgrade | Op::Remove => cli.nodeps += 1,
                            _ => {}
                        },
                        'e' => {
                            if cli.op == Op::Query {
                                cli.q_explicit = true
                            }
                        }
                        't' => {
                            if cli.op == Op::Query {
                                cli.q_unreq = true
                            }
                        }
                        'k' => match cli.op {
                            Op::Query => cli.q_file_check = true,
                            Op::Database => cli.db_check = true,
                            _ => {}
                        },
                        'q' => match cli.op {
                            Op::Sync => cli.s_quiet = true,
                            Op::Query => cli.q_quiet = true,
                            Op::Database => cli.q_quiet = true,
                            _ => {}
                        },
                        'o' => {
                            if cli.op == Op::Query {
                                cli.q_owns = true
                            }
                        }
                        'm' => {
                            if cli.op == Op::Query {
                                cli.q_foreign = true
                            }
                        }
                        'p' => match cli.op {
                            Op::Query => cli.q_file_query = true,
                            _ => cli.print = true,
                        },
                        'v' => cli.verbose += 1,
                        'V' => cli.version = true,
                        'h' => cli.help = true,
                        'T' => cli.deptest = true,
                        _ => return Err(format!("unsupported flag: -{c}")),
                    }
                }
            } else {
                cli.targets.push(arg.clone());
            }
            i += 1;
        }

        if let Some(color) = &cli.color {
            cli.plain = match color.as_str() {
                "always" => false,
                "never" => true,
                "auto" => !std::io::stdout().is_terminal(),
                _ => {
                    return Err(format!(
                        "invalid value for --color: {color}. Expected always, never, or auto"
                    ));
                }
            };
        }

        Ok(cli)
    }
}

// ── pacman.conf parser ────────────────────────────────────────────────────────

fn conf_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    Some(rest.trim())
}

fn collect_servers_recursive(
    lines: &[String],
    repo: &str,
    depth: u8,
    config_path: &str,
) -> Vec<String> {
    if depth > 10 {
        return Vec::new();
    }
    let arch = std::env::consts::ARCH;
    let mut servers = Vec::new();

    for line in lines {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            // In an included file, we might have new sections.
            // We only care about Servers in the current context.
        }
        if let Some(url) = conf_value(line, "Server") {
            servers.push(url.replace("$repo", repo).replace("$arch", arch));
        } else if let Some(path) = conf_value(line, "Include") {
            let full_path = if path.starts_with('/') {
                std::path::PathBuf::from(path)
            } else {
                std::path::Path::new(config_path)
                    .parent()
                    .unwrap()
                    .join(path)
            };
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let inc_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                servers.extend(collect_servers_recursive(
                    &inc_lines,
                    repo,
                    depth + 1,
                    config_path,
                ));
            }
        }
    }
    servers
}

fn collect_servers(
    conf_lines: &[&str],
    start: usize,
    repo: &str,
    config_path: &str,
) -> Vec<String> {
    let mut servers = Vec::new();
    for line in conf_lines[start..].iter() {
        let line = line.trim();
        if line.starts_with('[') {
            break;
        }
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(url) = conf_value(line, "Server") {
            let arch = std::env::consts::ARCH;
            servers.push(url.replace("$repo", repo).replace("$arch", arch));
        } else if let Some(path) = conf_value(line, "Include") {
            let full_path = if path.starts_with('/') {
                std::path::PathBuf::from(path)
            } else {
                std::path::Path::new(config_path)
                    .parent()
                    .unwrap()
                    .join(path)
            };
            if let Ok(content) = std::fs::read_to_string(&full_path) {
                let inc_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                servers.extend(collect_servers_recursive(&inc_lines, repo, 1, config_path));
            }
        }
    }
    servers
}

/// Parse a space-separated option value (e.g. "IgnorePkg = foo bar baz")
/// and split it into individual tokens. Multiple occurrences of the same
/// key are accumulated — pacman.conf allows repeated lines.
fn collect_option_list<'a>(lines: &[&'a str], key: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut in_options = false;
    for line in lines {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_options = &line[1..line.len() - 1] == "options";
            continue;
        }
        if !in_options || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(val) = conf_value(line, key) {
            out.extend(val.split_whitespace().map(|s| s.to_string()));
        }
    }
    out
}

fn parse_siglevel(val: &str) -> SigLevel {
    match val.to_lowercase().as_str() {
        "none" => SigLevel::NONE,
        _ => SigLevel::USE_DEFAULT,
    }
}

fn register_sync_dbs(handle: &mut Alpm, cfg: &config::ResolvedConfig, config_path: &str) {
    let raw = std::fs::read_to_string(config_path).unwrap_or_default();
    let lines: Vec<&str> = raw.lines().collect();

    // ── Apply [options] settings that libalpm needs to behave like pacman ──
    for pkg in collect_option_list(&lines, "IgnorePkg") {
        handle.add_ignorepkg(pkg.as_str()).ok();
    }
    for grp in collect_option_list(&lines, "IgnoreGroup") {
        handle.add_ignoregroup(grp.as_str()).ok();
    }
    for path in collect_option_list(&lines, "NoUpgrade") {
        handle.add_noupgrade(path.as_str()).ok();
    }
    for path in collect_option_list(&lines, "NoExtract") {
        handle.add_noextract(path.as_str()).ok();
    }

    // Additional options we can apply to the handle
    if let Some(val) = lines
        .iter()
        .find_map(|l| conf_value(l, "ParallelDownloads"))
    {
        if let Ok(n) = val.parse::<u32>() {
            handle.set_parallel_downloads(n);
        }
    }
    if let Some(val) = lines.iter().find_map(|l| conf_value(l, "CheckSpace")) {
        let check = val.to_lowercase() == "yes";
        handle.set_check_space(check);
    }
    if let Some(val) = lines
        .iter()
        .find_map(|l| conf_value(l, "DisableDownloadTimeout"))
    {
        let disable = val.to_lowercase() == "yes";
        handle.set_disable_dl_timeout(disable);
    }

    // ── Register sync databases ────────────────────────────────────────────
    let sig = if let Some(val) = lines.iter().find_map(|l| conf_value(l, "SigLevel")) {
        parse_siglevel(val)
    } else {
        SigLevel::USE_DEFAULT
    };
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if !line.starts_with('[') || !line.ends_with(']') {
            continue;
        }
        let name = &line[1..line.len() - 1];
        if name == "options" {
            continue;
        }
        let servers = collect_servers(&lines, i + 1, name, config_path);
        match handle.register_syncdb_mut(name, sig) {
            Ok(db) => {
                for s in &servers {
                    db.add_server(s.as_str()).ok();
                }
            }
            Err(e) => warn(&format!("could not register repo '{name}': {e}"), cfg),
        }
    }
}

// ── Handle ────────────────────────────────────────────────────────────────────

fn make_handle(cli: &Cli, cfg: config::ResolvedConfig) -> (Alpm, config::ResolvedConfig) {
    let root = cli.root.as_deref().unwrap_or(ROOT);
    let dbpath = cli.dbpath.as_deref().unwrap_or(DBPATH);

    let mut handle = Alpm::new(root, dbpath).unwrap_or_else(|e| {
        error(&format!("failed to init alpm: {e}"), &cfg);
        process::exit(1);
    });
    handle
        .set_logfile(cli.logfile.as_deref().unwrap_or(LOGFILE))
        .ok();
    handle
        .set_gpgdir(cli.gpgdir.as_deref().unwrap_or(GPGDIR))
        .ok();
    if let Some(_hookdir) = &cli.hookdir {
        // handle.set_hookdirs(AlpmList::from_iter(vec![hookdir.as_str()])).ok();
    }
    if let Some(arch) = &cli.arch {
        handle
            .set_architectures(AlpmListMut::from_iter([arch.as_str()]))
            .ok();
    }

    if !cli.cachedirs.is_empty() {
        for d in &cli.cachedirs {
            handle.add_cachedir(d.as_str()).ok();
        }
    } else {
        for d in CACHEDIRS {
            handle.add_cachedir(*d).ok();
        }
    }

    let cfg_path = cli.config.as_deref().unwrap_or("/etc/pacman.conf");
    register_sync_dbs(&mut handle, &cfg, cfg_path);

    // Now apply CLI-specific ignore overrides (these should override pacman.conf)
    for pkg in &cli.ignorepkg {
        handle.add_ignorepkg(pkg.as_str()).ok();
    }
    for grp in &cli.ignoregroup {
        handle.add_ignoregroup(grp.as_str()).ok();
    }
    for pattern in &cli.overwrite {
        handle.add_overwrite_file(pattern.as_str()).ok();
    }
    for assumed in &cli.assume_installed {
        let dep = alpm::Depend::new(assumed.clone());
        handle.add_assume_installed(&dep).ok();
    }

    handle.set_log_cb(cfg.clone(), callbacks::log_cb);
    handle.set_event_cb(cfg.clone(), callbacks::event_cb);
    handle.set_progress_cb(cfg.clone(), callbacks::progress_cb);
    handle.set_dl_cb(cfg.clone(), callbacks::dl_cb);
    handle.set_question_cb(cfg.clone(), callbacks::question_cb);
    (handle, cfg)
}

// ── Sync (-S) ─────────────────────────────────────────────────────────────────

fn do_sync_info(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        error("no targets specified for sync info", cfg);
        process::exit(1);
    }
    for name in &cli.targets {
        let mut found = false;
        if !cli.s_aur_only {
            if let Some(p) = handle.syncdbs().find_satisfier(name.as_str()) {
                if cli.s_quiet {
                    println!("{}", p.name());
                } else {
                    print_pkg_info_sync(p, cfg);
                }
                found = true;
            }
        }

        if !found && !cli.s_sync_only {
            // Fallback to AUR
            let aur = aur::AurClient::new();
            match aur.get_info(name) {
                Ok(p) => {
                    println!();
                    kv("Name", &p.name, cfg);
                    kv("Version", &p.version, cfg);
                    kv("Description", p.description.as_deref().unwrap_or("—"), cfg);
                    kv("URL", p.url.as_deref().unwrap_or("—"), cfg);
                    kv("Maintainer", p.maintainer.as_deref().unwrap_or("—"), cfg);
                    println!();
                    found = true;
                }
                Err(_) => {}
            }
        }

        if !found {
            error(&format!("package not found in databases (AUR: {}, Sync: {})", cli.s_aur_only, !cli.s_sync_only), cfg);
            // Actually, just a simple error is better
            error(&format!("package not found: {name}"), cfg);
        }
    }
}

fn do_sync_search(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        warn("no search terms given", cfg);
        return;
    }
    let mut any = false;
    if !cli.s_aur_only {
        for db in handle.syncdbs() {
            for p in db.pkgs() {
                let name = p.name().to_lowercase();
                let desc = p.desc().unwrap_or("").to_lowercase();
                if cli.targets.iter().all(|t| {
                    let t = t.to_lowercase();
                    name.contains(t.as_str()) || desc.contains(t.as_str())
                }) {
                    if cli.s_quiet {
                        println!("{}", p.name());
                    } else if cfg.plain {
                        println!(
                            "{}/{} {} - {}",
                            db.name(),
                            p.name(),
                            p.version(),
                            p.desc().unwrap_or("")
                        );
                    } else {
                        println!(
                            "  {GREEN}{repo}/{name}{RST} {DIM}{ver}{RST} {SUBTEXT1}- {desc}{RST}",
                            repo = db.name(),
                            name = p.name(),
                            ver = p.version(),
                            desc = p.desc().unwrap_or("")
                        );
                    }
                    any = true;
                }
            }
        }
    }

    // ── AUR Search ──────────────────────────────────────────────────────────────────
    if !cli.s_sync_only {
        let aur = aur::AurClient::new();
        for term in &cli.targets {
            match aur.search(term) {
                Ok(results) => {
                    for p in results {
                        if cli.s_quiet {
                            println!("{}", p.name);
                        } else if cfg.plain {
                            println!("aur/{} {} - {}", p.name, p.version, p.description.as_deref().unwrap_or(""));
                        } else {
                            println!(
                                "  {MAUVE}aur/{name}{RST} {DIM}{ver}{RST} {SUBTEXT1}- {desc}{RST}",
                                name = p.name,
                                ver = p.version,
                                desc = p.description.as_deref().unwrap_or("")
                            );
                        }
                        any = true;
                    }
                }
                Err(e) => {
                    warn(&format!("AUR search failed for {term}: {e}"), cfg);
                }
            }
        }
    }

    if !any {
        info("no matching packages found", cfg);
    }
}

fn do_sync_list(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        for db in handle.syncdbs() {
            if !cli.s_quiet {
                if cfg.plain {
                    println!("\nRepository {}:", db.name());
                } else {
                    println!("\n{MAUVE}{BOLD}Repository {}:{RST}", db.name());
                }
            }
            for p in db.pkgs() {
                if cli.s_quiet {
                    println!("{}", p.name());
                } else if cfg.plain {
                    println!("  {:<30} {}", p.name(), p.version());
                } else {
                    println!("  {TEXT}{:<30}{RST} {DIM}{}{RST}", p.name(), p.version());
                }
            }
        }
    } else {
        for repo_name in &cli.targets {
            if let Some(db) = handle.syncdbs().iter().find(|db| db.name() == repo_name) {
                if !cli.s_quiet {
                    if cfg.plain {
                        println!("\nRepository {}:", db.name());
                    } else {
                        println!("\n{MAUVE}{BOLD}Repository {}:{RST}", db.name());
                    }
                }
                for p in db.pkgs() {
                    if cli.s_quiet {
                        println!("{}", p.name());
                    } else if cfg.plain {
                        println!("  {:<30} {}", p.name(), p.version());
                    } else {
                        println!("  {TEXT}{:<30}{RST} {DIM}{}{RST}", p.name(), p.version());
                    }
                }
            } else {
                error(&format!("repository not found: {repo_name}"), cfg);
            }
        }
    }
}

fn do_sync_groups(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        error("no group specified for sync groups", cfg);
        process::exit(1);
    }
    for group_name in &cli.targets {
        let mut members = Vec::new();
        for db in handle.syncdbs() {
            for p in db.pkgs() {
                if p.groups().iter().any(|g| g == group_name) {
                    members.push(format!("{}/{}", db.name(), p.name()));
                }
            }
        }
        if members.is_empty() {
            error(&format!("group not found: {group_name}"), cfg);
        } else {
            if !cli.s_quiet {
                if cfg.plain {
                    println!("{group_name}:");
                } else {
                    println!("{MAUVE}{BOLD}{group_name}:{RST}");
                }
            }
            for m in members {
                if cfg.plain {
                    println!("  {}", m);
                } else {
                    println!("  {TEXT}{}{RST}", m);
                }
            }
        }
    }
}

fn do_sync_clean(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if !cli.s_quiet {
        header("cleaning package cache", cfg);
    }
    let mut removed_count = 0;
    for dir in handle.cachedirs() {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    let filename = path.file_name().unwrap().to_string_lossy();
                    if filename.contains(".pkg.tar.") {
                        let name = filename.split('-').next().unwrap_or("");
                        if handle.localdb().pkg(name).is_err() {
                            if std::fs::remove_file(&path).is_ok() {
                                removed_count += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    if !cli.s_quiet {
        success(
            &format!("removed {} unused packages from cache", removed_count),
            cfg,
        );
    }
}

// ── FFI for files database (not in alpm crate) ───────────────────────────────────
#[repr(C)]
struct AlpmListRaw {
    data: *mut std::os::raw::c_void,
    prev: *mut AlpmListRaw,
    next: *mut AlpmListRaw,
}

extern "C" {
    fn alpm_files_update(handle: *mut std::os::raw::c_void, force: i32) -> i32;
    fn alpm_files_search(
        handle: *mut std::os::raw::c_void,
        needle: *const std::os::raw::c_char,
        ret: *mut *mut AlpmListRaw,
    ) -> i32;
    fn alpm_list_free(list: *mut AlpmListRaw);
    fn alpm_pkg_get_name(pkg: *mut std::os::raw::c_void) -> *const std::os::raw::c_char;
    fn alpm_pkg_get_version(pkg: *mut std::os::raw::c_void) -> *const std::os::raw::c_char;
}

fn do_files(handle: &mut Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        error("no targets specified for files operation", cfg);
        process::exit(1);
    }

    if cli.refresh > 0 {
        header("updating files database", cfg);
        let force = cli.refresh > 1;
        unsafe {
            if alpm_files_update(handle.as_alpm_handle_t() as *mut _, force as i32) != 0 {
                error("failed to update files database", cfg);
            } else {
                success("files database updated", cfg);
            }
        }
    }

    for target in &cli.targets {
        let needle = std::ffi::CString::new(target.as_str()).unwrap();
        let mut ret = std::ptr::null_mut();

        unsafe {
            if alpm_files_search(
                handle.as_alpm_handle_t() as *mut _,
                needle.as_ptr(),
                &mut ret,
            ) == 0
            {
                if ret.is_null() {
                    error(&format!("no package found owning {target}"), cfg);
                } else {
                    let mut curr = ret;
                    while !curr.is_null() {
                        let pkg_ptr = (*curr).data;
                        let name =
                            std::ffi::CStr::from_ptr(alpm_pkg_get_name(pkg_ptr)).to_string_lossy();
                        let ver = std::ffi::CStr::from_ptr(alpm_pkg_get_version(pkg_ptr))
                            .to_string_lossy();
                        if cfg.plain {
                            println!("  {:<30} {}", name, ver);
                        } else {
                            println!("  {TEXT}{:<30}{RST} {DIM}{}{RST}", name, ver);
                        }
                        curr = (*curr).next;
                    }
                    alpm_list_free(ret);
                }
            } else {
                error(
                    &format!("failed to search files database for {target}"),
                    cfg,
                );
            }
        }
    }
}

fn print_pkg_info_sync(pkg: &alpm::Package, cfg: &config::ResolvedConfig) {
    println!();
    kv("Name", pkg.name(), cfg);
    kv("Version", pkg.version().as_str(), cfg);
    kv("Description", pkg.desc().unwrap_or("—"), cfg);
    kv("Architecture", pkg.arch().unwrap_or("—"), cfg);
    kv("URL", pkg.url().unwrap_or("—"), cfg);
    kv(
        "Licenses",
        &pkg.licenses()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("  "),
        cfg,
    );
    kv(
        "Groups",
        &pkg.groups()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join("  "),
        cfg,
    );
    kv(
        "Depends On",
        &pkg.depends()
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("  "),
        cfg,
    );
    kv(
        "Optional Deps",
        &pkg.optdepends()
            .iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("  "),
        cfg,
    );
    kv("Install Size", &human_size(pkg.isize()), cfg);
    println!();
}

fn do_sync(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool, cfg: &config::ResolvedConfig) {
    if cli.refresh > 0 {
        check_root(cfg);
        header("synchronising package databases", cfg);
        let force = cli.refresh > 1;
        match handle.syncdbs_mut().update(force) {
            Ok(false) => info("all databases are up to date", cfg),
            Ok(true) => success("databases updated", cfg),
            Err(e) => warn(&format!("some mirrors failed ({}); continuing", e), cfg),
        }
    }

    if cli.s_info || cli.s_search || cli.s_list || cli.s_groups {
        if cli.s_info {
            do_sync_info(handle, cli, cfg);
        }
        if cli.s_search {
            do_sync_search(handle, cli, cfg);
        }
        if cli.s_list {
            do_sync_list(handle, cli, cfg);
        }
        if cli.s_groups {
            do_sync_groups(handle, cli, cfg);
        }
        return;
    }

    if cli.s_clean {
        do_sync_clean(handle, cli, cfg);
        return;
    }

    if cli.targets.is_empty() && cli.sysupgrade == 0 {
        return;
    }

    if cli.sysupgrade > 0 {
        header("starting full system upgrade", cfg);

        let mut upgrade_flags = TransFlag::NONE;
        if cli.downloadonly {
            upgrade_flags |= TransFlag::DOWNLOAD_ONLY;
        }
        if cli.nodeps > 0 {
            upgrade_flags |= TransFlag::NO_DEPS;
        }
        if cli.dbonly {
            upgrade_flags |= TransFlag::DB_ONLY;
        }
        if cli.noscriptlet {
            upgrade_flags |= TransFlag::NO_SCRIPTLET;
        }
        check_root(cfg);
        handle.trans_init(upgrade_flags).unwrap_or_else(|e| {
            error(
                &format!("failed to init transaction for sysupgrade: {e}"),
                cfg,
            );
            process::exit(1);
        });

        handle
            .sync_sysupgrade(cli.sysupgrade >= 2)
            .unwrap_or_else(|e| {
                error(&format!("sysupgrade failed: {e}"), cfg);
                process::exit(1);
            });
    }

    if cli.sysupgrade == 0 {
        let mut flags = TransFlag::NONE;
        if cli.downloadonly {
            flags |= TransFlag::DOWNLOAD_ONLY;
        }
        if cli.nodeps > 0 {
            flags |= TransFlag::NO_DEPS;
        }
        if cli.dbonly {
            flags |= TransFlag::DB_ONLY;
        }
        if cli.noscriptlet {
            flags |= TransFlag::NO_SCRIPTLET;
        }
    }

    // ── Resolve targets (Sync vs AUR) ─────────────────────────────────────────────
    let mut sync_pkgs = Vec::new();
    let mut aur_pkgs = Vec::new();

    for target in &cli.targets {
        let mut matched_sync = false;
        if !cli.s_aur_only {
            if let Some(p) = handle.syncdbs().find_satisfier(target.as_str()) {
                sync_pkgs.push(p.name().to_string());
                matched_sync = true;
            } else {
                // Check if it's a group in sync dbs
                for db in handle.syncdbs() {
                    for p in db.pkgs() {
                        if p.groups().iter().any(|g| g == target) {
                            sync_pkgs.push(p.name().to_string());
                            matched_sync = true;
                        }
                    }
                }
            }
        }

        if !matched_sync && !cli.s_sync_only {
            aur_pkgs.push(target.clone());
        }
    }

    // 1. Build AUR Packages FIRST (before locking the DB)
    let mut built_aur_paths = Vec::new();
    if !aur_pkgs.is_empty() {
        let build_mgr = aur::BuildManager::new().unwrap_or_else(|e| {
            error(&format!("AUR build error: {e}"), cfg);
            process::exit(1);
        });

        for name in &aur_pkgs {
            match build_mgr.build_and_install(name, cfg) {
                Ok(path) => {
                    built_aur_paths.push((name.clone(), path));
                }
                Err(e) => {
                    error(&format!("AUR build failed for {name}: {e}"), cfg);
                    process::exit(1);
                }
            }
        }
    }

    // 2. Now initialize the transaction (Locks the DB)
    if cli.sysupgrade == 0 {
        check_root(cfg);
        let mut flags = TransFlag::NONE;
        if cli.downloadonly {
            flags |= TransFlag::DOWNLOAD_ONLY;
        }
        if cli.nodeps > 0 {
            flags |= TransFlag::NO_DEPS;
        }
        if cli.dbonly {
            flags |= TransFlag::DB_ONLY;
        }
        if cli.noscriptlet {
            flags |= TransFlag::NO_SCRIPTLET;
        }

        check_root(cfg);
        check_root(cfg);
        handle.trans_init(flags).unwrap_or_else(|e| {
            error(&format!("failed to init transaction: {e}"), cfg);
            process::exit(1);
        });
    }

    // 3. Add Sync Packages
    for name in &sync_pkgs {
        if let Some(p) = handle.syncdbs().find_satisfier(name.as_str()) {
            if cli.needed {
                if let Ok(local_pkg) = handle.localdb().pkg(name.as_str()) {
                    if alpm::vercmp(local_pkg.version().as_str(), p.version().as_str())
                        != std::cmp::Ordering::Less
                    {
                        continue;
                    }
                }
            }
            if let Err(e) = handle.trans_add_pkg(p) {
                error(&format!("could not add {name}: {e}"), cfg);
                handle.trans_release().ok();
                process::exit(1);
            }
        }
    }

    // 4. Add built AUR Packages
    for (name, path) in built_aur_paths {
        let path_str = path.to_str().unwrap().to_string();

        let res = {
            let pkg_res = handle.pkg_load(path_str.as_bytes(), true, SigLevel::USE_DEFAULT);
            match pkg_res {
                Ok(p) => {
                    if let Err(e) = handle.trans_add_pkg(p) {
                        Err(format!("could not add AUR package {name}: {e}"))
                    } else {
                        Ok(())
                    }
                }
                Err(e) => Err(format!("failed to load built package {name}: {e}")),
            }
        };

        if let Err(e) = res {
            error(&e, cfg);
            handle.trans_release().ok();
            process::exit(1);
        }
    }

    if handle.trans_add().is_empty() && handle.trans_remove().is_empty() {
        success("no packages to install or upgrade", cfg);
        handle.trans_release().ok();
        return;
    }

    trans_prepare_or_die(handle, cfg);

    if let Some(fmt) = &cli.print_format {
        for p in handle.trans_add() {
            let mut line = format_pkg(p, fmt);
            // Handle %r (repo)
            if line.contains("%r") {
                let repo = handle
                    .syncdbs()
                    .iter()
                    .find(|db| db.pkg(p.name()).is_ok())
                    .map(|db| db.name())
                    .unwrap_or("unknown");
                line = line.replace("%r", repo);
            }
            println!("{}", line);
        }
        handle.trans_release().ok();
        process::exit(0);
    }

    print_sync_summary(handle, cfg);

    if cli.print {
        handle.trans_release().ok();
        return;
    }

    if !cli.noconfirm && !confirm("proceed with installation?", true, cfg) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cfg);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cfg);

    if cli.asdeps || cli.asexplicit {
        let reason = if cli.asdeps {
            PackageReason::Depend
        } else {
            PackageReason::Explicit
        };
        let names: Vec<String> = handle
            .trans_add()
            .iter()
            .map(|p| p.name().to_string())
            .collect();
        handle.trans_release().ok();
        for name in names {
            if let Ok(pkg) = handle.localdb().pkg(name.as_str()) {
                pkg.set_reason(reason).ok();
            }
        }
    } else {
        handle.trans_release().ok();
    }

    success("transaction complete", cfg);
}

// ── Remove (-R) ───────────────────────────────────────────────────────────────

fn do_remove(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool, cfg: &config::ResolvedConfig) {
    let mut flags = TransFlag::NONE;
    if cli.nosave {
        flags |= TransFlag::NO_SAVE;
    }
    if cli.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if cli.recursive >= 2 {
        flags |= TransFlag::RECURSE_ALL;
    } else if cli.recursive == 1 {
        flags |= TransFlag::RECURSE;
    }

    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cfg);
        process::exit(1);
    });

    if cli.targets.is_empty() {
        warn("no targets specified for removal", cfg);
        handle.trans_release().ok();
        return;
    }

    let mut missing = Vec::new();
    for name in &cli.targets {
        if handle.localdb().pkg(name.as_str()).is_err() {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        for m in &missing {
            error(&format!("target not found: {m}"), cfg);
        }
        handle.trans_release().ok();
        process::exit(1);
    }

    for name in &cli.targets {
        let pkg = handle.localdb().pkg(name.as_str()).unwrap();

        if cli.unneeded && !pkg.required_by().is_empty() {
            warn(
                &format!("skipping {name}: it is required by other packages"),
                cfg,
            );
            continue;
        }

        if cli.cascade {
            // Recursive search for all packages that depend on this one.
            let mut to_remove = std::collections::HashSet::new();
            let mut stack = vec![pkg.name().to_string()];
            while let Some(current_name) = stack.pop() {
                if to_remove.insert(current_name.clone()) {
                    if let Ok(p) = handle.localdb().pkg(current_name.as_str()) {
                        for dependent in p.required_by() {
                            stack.push(dependent.to_string());
                        }
                    }
                }
            }
            for rem_name in to_remove {
                if let Ok(rem_pkg) = handle.localdb().pkg(rem_name.as_str()) {
                    handle.trans_remove_pkg(rem_pkg).ok();
                }
            }
        } else {
            handle.trans_remove_pkg(pkg).unwrap_or_else(|e| {
                error(&format!("could not queue {name} for removal: {e}"), cfg);
                process::exit(1);
            });
        }
    }

    trans_prepare_or_die(handle, cfg);
    print_remove_summary(handle, cfg);

    if cli.print {
        handle.trans_release().ok();
        return;
    }

    if !cli.noconfirm && !confirm("proceed with removal?", true, cfg) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cfg);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cfg);
    handle.trans_release().ok();
    success("removal complete", cfg);
}

// ── Upgrade (-U) ──────────────────────────────────────────────────────────────

fn do_upgrade(
    handle: &mut Alpm,
    cli: &Cli,
    interrupted: &AtomicBool,
    cfg: &config::ResolvedConfig,
) {
    let mut flags = TransFlag::NONE;
    if cli.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if cli.dbonly {
        flags |= TransFlag::DB_ONLY;
    }
    if cli.noscriptlet {
        flags |= TransFlag::NO_SCRIPTLET;
    }
    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cfg);
        process::exit(1);
    });

    if cli.targets.is_empty() {
        warn("no targets specified for upgrade", cfg);
        handle.trans_release().ok();
        return;
    }

    for path in &cli.targets {
        match handle.pkg_load(path.as_str(), true, SigLevel::USE_DEFAULT) {
            Ok(pkg) => {
                handle.trans_add_pkg(pkg).unwrap_or_else(|e| {
                    error(&format!("could not add {path}: {e}"), cfg);
                    process::exit(1);
                });
            }
            Err(e) => {
                error(&format!("failed to load {path}: {e}"), cfg);
                process::exit(1);
            }
        }
    }

    trans_prepare_or_die(handle, cfg);

    if let Some(fmt) = &cli.print_format {
        for p in handle.trans_add() {
            let mut line = format_pkg(p, fmt);
            // Handle %r (repo)
            if line.contains("%r") {
                let repo = handle
                    .syncdbs()
                    .iter()
                    .find(|db| db.pkg(p.name()).is_ok())
                    .map(|db| db.name())
                    .unwrap_or("unknown");
                line = line.replace("%r", repo);
            }
            println!("{}", line);
        }
        handle.trans_release().ok();
        process::exit(0);
    }

    print_sync_summary(handle, cfg);

    if cli.print {
        handle.trans_release().ok();
        return;
    }

    if !cli.noconfirm && !confirm("proceed with installation?", true, cfg) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cfg);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cfg);
    handle.trans_release().ok();
    success("upgrade complete", cfg);
}

// ── Database (-D) ─────────────────────────────────────────────────────────────

fn do_database(handle: &Alpm, cli: &Cli, interrupted: &AtomicBool, cfg: &config::ResolvedConfig) {
    if cli.db_check {
        if !cli.q_quiet {
            header("checking local database consistency", cfg);
            // libalpm's db_check (la_db_check) returns a result.
            // For now, we'll just print that this is not fully implemented.
            info(
                "database consistency check is not fully implemented in libalpm bindings",
                cfg,
            );
        }
        return;
    }

    let reason = if cli.asdeps {
        Some(PackageReason::Depend)
    } else if cli.asexplicit {
        Some(PackageReason::Explicit)
    } else {
        None
    };

    if let Some(r) = reason {
        for name in &cli.targets {
            if interrupted.load(Ordering::SeqCst) {
                println!();
                warn("interrupted — remaining packages were not updated", cfg);
                process::exit(130);
            }
            match handle.localdb().pkg(name.as_str()) {
                Ok(pkg) => {
                    check_root(cfg);
                    pkg.set_reason(r).unwrap_or_else(|e| {
                        error(&format!("could not set reason for {name}: {e}"), cfg);
                    });
                    let label = if r == PackageReason::Depend {
                        "dependency"
                    } else {
                        "explicit"
                    };
                    if !cli.q_quiet {
                        info(&format!("{name}: install reason set to '{label}'"), cfg);
                    }
                }
                Err(_) => error(&format!("package not found: {name}"), cfg),
            }
        }
    } else if !cli.q_quiet {
        warn("no --asdeps or --asexplicit flag given; nothing to do", cfg);
    }
}

// ── Query (-Q) ────────────────────────────────────────────────────────────────

fn do_query(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.q_owns && !cli.targets.is_empty() {
        for t in &cli.targets {
            query_owns(handle, t, cfg);
        }
        return;
    }
    if cli.q_search {
        query_search(handle, &cli.targets, cfg);
        return;
    }
    if cli.q_file_query && !cli.targets.is_empty() {
        // -Qp: query package by file path
        // Since we don't have a dedicated file DB helper implemented yet,
        // we can simulate this by searching all packages for the file.
        for file_path in &cli.targets {
            let needle = file_path.trim_start_matches('/');
            let mut found = false;
            for db in handle.syncdbs() {
                for pkg in db.pkgs() {
                    for f in pkg.files().files() {
                        if String::from_utf8_lossy(f.name()).trim_start_matches('/') == needle {
                            if cfg.plain {
                                println!(
                                    "  {} would be owned by {} {}",
                                    file_path,
                                    pkg.name(),
                                    pkg.version()
                                );
                            } else {
                                println!(
                                    "  {TEXT}{}{RST} would be owned by {GREEN}{} {}{RST}",
                                    file_path,
                                    pkg.name(),
                                    pkg.version()
                                );
                            }
                            found = true;
                        }
                    }
                }
            }
            if !found {
                error(&format!("no package found that owns {file_path}"), cfg);
            }
        }
        return;
    }
    let opts = QueryOpts {
        info: cli.q_info,
        deps: cli.q_deps,
        explicit: cli.q_explicit,
        unrequired: cli.q_unreq,
        upgrades: cli.q_upgrades,
        file_check: cli.q_file_check,
        foreign: cli.q_foreign,
        native: cli.q_native,
        list: cli.q_list,
        groups: cli.q_groups,
        changelog: cli.q_changelog,
        quiet: cli.q_quiet,
    };
    query(handle, &cli.targets, &opts, cfg);
}

// ── Transaction helpers ───────────────────────────────────────────────────────

fn trans_prepare_or_die(handle: &mut Alpm, cfg: &config::ResolvedConfig) {
    // PrepareError borrows from handle, so we collect everything into owned
    // Strings inside a nested block, letting `e` drop before we touch handle.
    enum PrepDiag {
        Unsatisfied(Vec<(String, Option<String>)>),
        Conflicting(Vec<(String, String, String)>),
        None,
    }
    let outcome: Option<(String, PrepDiag)> = match handle.trans_prepare() {
        Ok(()) => Option::None,
        Err(e) => {
            let msg = e.to_string();
            let diag = match e.data() {
                Some(alpm::PrepareData::UnsatisfiedDeps(list)) => PrepDiag::Unsatisfied(
                    list.iter()
                        .map(|dep| {
                            let cause = dep.causing_pkg().map(|p| p.to_string());
                            (dep.depend().to_string(), cause)
                        })
                        .collect(),
                ),
                Some(alpm::PrepareData::ConflictingDeps(list)) => PrepDiag::Conflicting(
                    list.iter()
                        .map(|c| {
                            (
                                c.package1().name().to_string(),
                                c.package2().name().to_string(),
                                c.reason().to_string(),
                            )
                        })
                        .collect(),
                ),
                _ => PrepDiag::None,
            };
            // `e` drops here — borrow on handle released before we return
            Some((msg, diag))
        }
    };

    if let Some((msg, diag)) = outcome {
        handle.trans_release().ok();
        error(&format!("could not prepare transaction: {msg}"), cfg);
        match diag {
            PrepDiag::Unsatisfied(deps) => {
                println!();
                for (dep, cause) in deps {
                    let cause_str = match cause {
                        Some(p) => format!(" (required by {p})"),
                        None => String::new(),
                    };
                    if cfg.plain {
                        println!("  ✗  missing dependency: {dep}{cause_str}");
                    } else {
                        println!(
                            "  {RED}✗{RST}  {TEXT}missing dependency:{RST} \
                             {YELLOW}{dep}{RST}{DIM}{cause_str}{RST}",
                        );
                    }
                }
                println!();
            }
            PrepDiag::Conflicting(conflicts) => {
                println!();
                for (p1, p2, reason) in conflicts {
                    if cfg.plain {
                        println!("  ✗  conflict: {p1} ↔ {p2}  ({reason})");
                    } else {
                        println!(
                            "  {RED}✗{RST}  {TEXT}conflict:{RST} \
                             {YELLOW}{p1}{RST}{DIM} ↔ {p2}{RST}  {DIM}({reason}){RST}",
                        );
                    }
                }
                println!();
            }
            PrepDiag::None => {}
        }
        process::exit(1);
    }
}

fn trans_commit_or_die(handle: &mut Alpm, interrupted: &AtomicBool, cfg: &config::ResolvedConfig) {
    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cfg);
        process::exit(130);
    }

    enum CommitDiag {
        FileConflict(Vec<(String, String)>),
        PkgInvalid(Vec<String>),
        None,
    }
    let outcome: Option<(String, CommitDiag)> = match handle.trans_commit() {
        Ok(()) => Option::None,
        Err(e) => {
            let msg = e.to_string();
            let diag = match e.data() {
                Some(alpm::CommitData::FileConflict(list)) => CommitDiag::FileConflict(
                    list.iter()
                        .map(|c| {
                            (
                                c.package1().name().to_string(),
                                c.package2().name().to_string(),
                            )
                        })
                        .collect(),
                ),
                Some(alpm::CommitData::PkgInvalid(list)) => {
                    CommitDiag::PkgInvalid(list.iter().map(|s| s.to_string()).collect())
                }
                None => CommitDiag::None,
            };
            // `e` drops here
            Some((msg, diag))
        }
    };

    if let Some((msg, diag)) = outcome {
        handle.trans_release().ok();
        println!();
        error(&format!("transaction failed: {msg}"), cfg);
        match diag {
            CommitDiag::FileConflict(conflicts) => {
                for (p1, p2) in conflicts {
                    if cfg.plain {
                        println!("  ✗  file conflict: {} ↔ {}", p1, p2);
                    } else {
                        println!(
                            "  {RED}✗{RST}  {TEXT}file conflict:{RST} \
                             {YELLOW}{p1}{RST}{DIM} ↔ {p2}{RST}",
                        );
                    }
                }
            }
            CommitDiag::PkgInvalid(names) => {
                for name in names {
                    if cfg.plain {
                        println!("  ✗  invalid package: {}", name);
                    } else {
                        println!("  {RED}✗{RST}  {TEXT}invalid package:{RST} {YELLOW}{name}{RST}");
                    }
                }
            }
            CommitDiag::None => {}
        }
        println!();
        process::exit(1);
    }
}

// ── Summaries ─────────────────────────────────────────────────────────────────

fn print_sync_summary(handle: &Alpm, cfg: &config::ResolvedConfig) {
    let add = handle.trans_add();
    let rem = handle.trans_remove();

    if !add.is_empty() {
        if cfg.plain {
            println!("\n  packages ({}):", add.len());
        } else {
            println!("\n  {MAUVE}{BOLD}packages ({}):{RST}", add.len());
        }
        let mut dl_total: i64 = 0;
        let mut inst_total: i64 = 0;
        let mut net_change: i64 = 0;

        for p in add.iter() {
            let local = handle.localdb().pkg(p.name());
            let is_up = local.is_ok();
            if cfg.plain {
                let ver_str = if let Ok(ref l) = local {
                    format!(" {} -> {}", l.version(), p.version())
                } else {
                    format!(" {}", p.version())
                };
                println!(
                    "    {} {}: {}  {}",
                    p.name(),
                    p.version(),
                    human_size(p.isize()),
                    ver_str
                );
            } else {
                let col = if is_up { BLUE } else { GREEN };
                let sym = if is_up { "⟳" } else { "↑" };
                let ver_str = if let Ok(ref l) = local {
                    format!(" {DIM}{} → {}{RST}", l.version(), p.version())
                } else {
                    format!(" {DIM}{}{RST}", p.version())
                };
                println!(
                    "    {col}{sym}{RST} {TEXT}{:<30}{RST}{ver_str}  {SUBTEXT1}{}{RST}",
                    p.name(),
                    human_size(p.isize()),
                );
            }
            let old_sz = local.as_ref().map(|l| l.isize()).unwrap_or(0);
            dl_total += p.download_size();
            inst_total += p.isize();
            net_change += p.isize() - old_sz;
        }

        println!();
        if cfg.plain {
            println!("  total download size:   {}", human_size(dl_total));
            println!("  total installed size:  {}", human_size(inst_total));
            if net_change != inst_total {
                println!("  net upgrade size:      {}", human_size(net_change));
            }
        } else {
            println!(
                "  {ROSEWATER}total download size:   {}{RST}",
                human_size(dl_total)
            );
            println!(
                "  {ROSEWATER}total installed size:  {}{RST}",
                human_size(inst_total)
            );
            if net_change != inst_total {
                println!(
                    "  {ROSEWATER}net upgrade size:      {}{RST}",
                    human_size(net_change)
                );
            }
        }
    }

    if !rem.is_empty() {
        if cfg.plain {
            println!("\n  removing ({}):", rem.len());
            for p in rem.iter() {
                println!("    - {}: {}", p.name(), p.version());
            }
        } else {
            println!("\n  {RED}{BOLD}removing ({}):{RST}", rem.len());
            for p in rem.iter() {
                println!(
                    "    {RED}✕{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}",
                    p.name(),
                    p.version()
                );
            }
        }
    }

    println!();
}

fn print_remove_summary(handle: &Alpm, cfg: &config::ResolvedConfig) {
    let rem = handle.trans_remove();
    if rem.is_empty() {
        warn("nothing to remove", cfg);
        return;
    }
    if cfg.plain {
        println!("\n  removing ({}):", rem.len());
        for p in rem.iter() {
            println!("    - {}: {}", p.name(), p.version());
        }
    } else {
        println!("\n  {RED}{BOLD}removing ({}):{RST}", rem.len());
        for p in rem.iter() {
            println!(
                "    {RED}✕{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}",
                p.name(),
                p.version()
            );
        }
    }
    let freed: i64 = rem.iter().map(|p| p.isize()).sum();
    if cfg.plain {
        println!("\n  freed space: {}", human_size(freed));
    } else {
        println!("\n  {ROSEWATER}freed space: {}{RST}", human_size(freed));
    }
    println!();
}

fn do_deptest(handle: &Alpm, cli: &Cli, cfg: &config::ResolvedConfig) {
    if cli.targets.is_empty() {
        error("no targets specified for dependency test", cfg);
        process::exit(1);
    }

    let mut unsatisfied = false;
    for dep_str in &cli.targets {
        if handle
            .localdb()
            .pkgs()
            .find_satisfier(dep_str.as_str())
            .is_none()
        {
            println!("{}", dep_str);
            unsatisfied = true;
        }
    }
    process::exit(if unsatisfied { 1 } else { 0 });
}

fn do_declarative(
    handle: &mut Alpm,
    cli: &Cli,
    interrupted: &AtomicBool,
    cfg: &config::ResolvedConfig,
) {
    let state_path = std::path::Path::new("/etc/pacwoman/packages.json");
    if !state_path.exists() {
        error(
            &format!("state file not found: {}", state_path.display()),
            cfg,
        );
        process::exit(1);
    }

    let content = std::fs::read_to_string(state_path).unwrap_or_else(|e| {
        error(&format!("could not read state file: {e}"), cfg);
        process::exit(1);
    });

    let desired: Vec<String> = serde_json::from_str(&content).unwrap_or_else(|e| {
        error(&format!("could not parse state file: {e}"), cfg);
        process::exit(1);
    });

    header("reconciling system state", cfg);

    let mut installed = std::collections::HashSet::new();
    for pkg in handle.localdb().pkgs() {
        installed.insert(pkg.name().to_string());
    }

    let desired_set: std::collections::HashSet<String> = desired.into_iter().collect();

    let to_install: Vec<String> = desired_set.difference(&installed).cloned().collect();
    let to_remove: Vec<String> = installed.difference(&desired_set).cloned().collect();

    if to_install.is_empty() && to_remove.is_empty() {
        success("system is in desired state", cfg);
        return;
    }

    let mut flags = TransFlag::NONE;
    if cli.nodeps > 0 {
        flags |= TransFlag::NO_DEPS;
    }
    if cli.dbonly {
        flags |= TransFlag::DB_ONLY;
    }
    if cli.noscriptlet {
        flags |= TransFlag::NO_SCRIPTLET;
    }
    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cfg);
        process::exit(1);
    });

    for pkg in &to_install {
        if let Some(p) = handle.syncdbs().find_satisfier(pkg.as_str()) {
            handle.trans_add_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not add {pkg}: {e}"), cfg);
                process::exit(1);
            });
        } else {
            error(&format!("package not found: {pkg}"), cfg);
            handle.trans_release().ok();
            process::exit(1);
        }
    }

    for pkg in &to_remove {
        // Protected packages: don't remove base or critical system components
        let is_protected = matches!(
            pkg.as_str(),
            "linux" | "pacman" | "glibc" | "systemd" | "bash"
        );
        if is_protected {
            error(
                &format!("protected package {pkg} is marked for removal"),
                cfg,
            );
            handle.trans_release().ok();
            process::exit(1);
        }
        if let Ok(p) = handle.localdb().pkg(pkg.as_str()) {
            handle.trans_remove_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not remove {pkg}: {e}"), cfg);
                process::exit(1);
            });
        }
    }

    trans_prepare_or_die(handle, cfg);

    if let Some(fmt) = &cli.print_format {
        for p in handle.trans_add() {
            let mut line = format_pkg(p, fmt);
            // Handle %r (repo)
            if line.contains("%r") {
                let repo = handle
                    .syncdbs()
                    .iter()
                    .find(|db| db.pkg(p.name()).is_ok())
                    .map(|db| db.name())
                    .unwrap_or("unknown");
                line = line.replace("%r", repo);
            }
            println!("{}", line);
        }
        handle.trans_release().ok();
        process::exit(0);
    }

    print_sync_summary(handle, cfg);

    if !cli.noconfirm && !confirm("proceed with reconciliation?", true, cfg) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cfg);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cfg);
    handle.trans_release().ok();
    success("declarative reconciliation complete", cfg);
}

fn is_lock_stale() -> bool {
    let lock_path = std::path::Path::new(DBPATH).join("db.lck");
    if !lock_path.exists() {
        return false;
    }

    if let Ok(pid_str) = std::fs::read_to_string(&lock_path) {
        if let Ok(pid) = pid_str.trim().parse::<libc::pid_t>() {
            // kill(pid, 0) checks if the process exists
            return unsafe {
                libc::kill(pid, 0) == -1
                    && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            };
        }
    }
    false
}

fn handle_stale_lock(cfg: &config::ResolvedConfig) {
    if is_lock_stale() {
        warn("a stale pacman lock was found", cfg);
        if confirm("remove it and proceed?", true, cfg) {
            let lock_path = std::path::Path::new(DBPATH).join("db.lck");
            if let Err(e) = std::fs::remove_file(lock_path) {
                error(&format!("could not remove lock: {e}"), cfg);
                process::exit(1);
            }
            success("lock removed", cfg);
        } else {
            process::exit(1);
        }
    }
}

fn confirm(msg: &str, default_yes: bool, cfg: &config::ResolvedConfig) -> bool {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    if cfg.plain {
        print!("\n  :: {msg} {hint} ");
    } else {
        print!("\n  {MAUVE}{BOLD}::{RST} {TEXT}{msg} {SUBTEXT1}{hint}{RST} ");
    }
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    let t = s.trim().to_lowercase();
    if t.is_empty() {
        default_yes
    } else {
        t == "y" || t == "yes"
    }
}

fn print_version() {
    println!("pacwoman v{}", CURRENT_VERSION);
}

fn format_pkg(pkg: &alpm::Package, fmt: &str) -> String {
    let mut res = fmt.to_string();
    res = res.replace("%n", pkg.name());
    res = res.replace("%v", pkg.version());
    res = res.replace("%s", &human_size(pkg.isize()));
    res = res.replace("%a", pkg.arch().unwrap_or("—"));
    res = res.replace("%l", pkg.desc().unwrap_or("—"));

    // Repo is harder because we need the syncdb
    res
}

fn print_help(op: &Op) {
    match op {
        Op::None => {
            println!("Usage: pacwoman [options] [targets]");
            println!("\nOptions:");
            println!("  -S, --sync         synchronize packages");
            println!("  -R, --remove      remove packages");
            println!("  -U, --upgrade    upgrade packages");
            println!("  -Q, --query       query packages");
            println!("  -D, --database    database operations");
            println!("  -V, --version     display version");
            println!("  -h, --help        display help");
        }
        Op::Sync => {
            println!("Usage: pacwoman -S [options] [targets]");
            println!("\nOptions:");
            println!("  -y, --refresh     update databases");
            println!("  -u, --sysupgrade  full system upgrade");
            println!("  -w, --downloadonly download only");
            println!("  -n, --nosave       do not save targets");
            println!("  -s, --search       search sync databases");
        }
        Op::Remove => {
            println!("Usage: pacwoman -R [options] [targets]");
            println!("\nOptions:");
            println!("  -s, --recursive    remove recursively");
            println!("  -n, --nosave       do not save targets");
        }
        Op::Upgrade => {
            println!("Usage: pacwoman -U [options] [targets]");
            println!("\nOptions:");
            println!("  -w, --downloadonly download only");
            println!("  -n, --nosave       do not save targets");
        }
        Op::Query => {
            println!("Usage: pacwoman -Q [options] [targets]");
            println!("\nOptions:");
            println!("  -i, --info        show package info");
            println!("  -d, --deps        show dependencies");
            println!("  -e, --explicit     show explicit packages");
            println!("  -t, --unrequired  show unrequired packages");
            println!("  -u, --upgrades    show upgradable packages");
            println!("  -k, --check       check file integrity");
            println!("  -q, --quiet       quiet output");
            println!("  -o, --owns        find package owning file");
            println!("  -s, --search      search local database");
        }
        Op::Database => {
            println!("Usage: pacwoman -D [options] [targets]");
            println!("\nOptions:");
            println!("  --asdeps          set install reason to dependency");
            println!("  --asexplicit      set install reason to explicit");
        }
        _ => {
            println!("Help not implemented for this operation.");
        }
    }
}

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn version_to_nums(v: &str) -> Vec<u32> {
    let mut nums = Vec::new();
    let mut current = String::new();
    for c in v.chars() {
        if c.is_numeric() {
            current.push(c);
        } else if !current.is_empty() {
            if let Ok(n) = current.parse() {
                nums.push(n);
            }
            current.clear();
        }
    }
    if !current.is_empty() {
        if let Ok(n) = current.parse() {
            nums.push(n);
        }
    }
    nums
}

fn is_version_greater(current: &str, remote: &str) -> bool {
    let c_parts = version_to_nums(current);
    let r_parts = version_to_nums(remote);

    for (c, r) in c_parts.iter().zip(r_parts.iter()) {
        if r > c {
            return true;
        }
        if r < c {
            return false;
        }
    }
    r_parts.len() > c_parts.len()
}

fn parse_cargo_version(toml: &str) -> Option<String> {
    let mut in_package = false;
    for line in toml.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            in_package = &trimmed[1..trimmed.len() - 1] == "package";
            continue;
        }
        if in_package {
            if trimmed.starts_with('[') {
                break; // reached next section
            }
            if let Some(val) = trimmed
                .strip_prefix("version = \"")
                .and_then(|s| s.strip_suffix('"'))
            {
                return Some(val.to_string());
            }
        }
    }
    None
}

fn check_for_update() -> Option<String> {
    // Best-effort: any failure is silently ignored so the tool always works
    // offline or on stale mirrors.
    let url = "https://raw.githubusercontent.com/Jlesster/pacwoman/main/Cargo.toml";

    let Ok(output) = std::process::Command::new("curl")
        .args(["-fsSL", "--max-time", "3", url])
        .output()
    else {
        return None;
    };

    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let remote_ver = parse_cargo_version(&body)?;

    if is_version_greater(CURRENT_VERSION, &remote_ver) {
        Some(remote_ver)
    } else {
        None
    }
}

fn find_cargo_path() -> Option<String> {
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            let candidate = format!("{dir}/cargo");
            if std::path::Path::new(&candidate).exists() {
                return Some(candidate);
            }
        }
    }

    if let Ok(user) = std::env::var("SUDO_USER") {
        let user_cargo = format!("/home/{user}/.cargo/bin/cargo");
        if std::path::Path::new(&user_cargo).exists() {
            return Some(user_cargo);
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        let candidate = format!("{home}/.cargo/bin/cargo");
        if std::path::Path::new(&candidate).exists() {
            return Some(candidate);
        }
    }

    None
}

fn build_with_cargo(project_dir: &std::path::Path, cfg: &config::ResolvedConfig) -> bool {
    let cargo_path = match find_cargo_path() {
        Some(p) => p,
        None => {
            error(
                "could not find cargo binary — check that Rust is installed and in PATH",
                cfg,
            );
            return false;
        }
    };

    info(&format!("building with: {cargo_path}"), cfg);

    let mut cmd = std::process::Command::new(&cargo_path);
    cmd.args(["build", "--release"]).current_dir(project_dir);

    if let Ok(user) = std::env::var("SUDO_USER") {
        let user_home = format!("/home/{user}");
        cmd.env("HOME", &user_home);
        cmd.env(
            "RUSTUP_TOOLCHAIN",
            std::env::var("RUSTUP_TOOLCHAIN").unwrap_or_else(|_| "stable".to_string()),
        );
    }

    cmd.status().map(|s| s.success()).unwrap_or(false)
}

fn perform_self_update(cfg: &config::ResolvedConfig) {
    let current_exe_path = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            error(&format!("could not determine current exe path: {e}"), cfg);
            return;
        }
    };

    // Resolve symlinks to ensure we are updating the actual binary, not a link
    let current_exe = std::fs::canonicalize(&current_exe_path).unwrap_or(current_exe_path);

    // Dev build path: if we are in target/release and a .git dir exists, just pull and build
    if current_exe.to_string_lossy().contains("target/release") {
        let mut project_root = current_exe
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent());
        let mut found_git = false;
        while let Some(root) = project_root {
            if root.join(".git").exists() {
                found_git = true;
                break;
            }
            project_root = root.parent();
        }

        if found_git {
            let root = project_root.unwrap();
            header("updating pacwoman (dev build - in-place)", cfg);

            info("pulling latest source...", cfg);
            let git_ok = std::process::Command::new("git")
                .arg("pull")
                .current_dir(&root)
                .status()
                .map(|s| s.success())
                .unwrap_or(false);

            if git_ok && build_with_cargo(&root, cfg) {
                success("pacwoman updated successfully", cfg);
                return;
            } else if !git_ok {
                error("failed to pull latest source", cfg);
                return;
            } else {
                error("failed to build pacwoman", cfg);
                return;
            }
        }
    }

    // Source-based update: clone, build, and install
    header("updating pacwoman (source-based)", cfg);

    let tmp_dir = std::env::temp_dir().join(format!("pacwoman-update-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        error(&format!("failed to create temp directory: {e}"), cfg);
        return;
    }

    let repo_url = "https://github.com/Jlesster/pacwoman.git";
    info(&format!("cloning repository to {tmp_dir:?}"), cfg);

    if std::process::Command::new("git")
        .args(["clone", repo_url, tmp_dir.to_str().unwrap()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        if build_with_cargo(&tmp_dir, cfg) {
            let built_bin = tmp_dir.join("target/release/pacwoman");

            // We use the `install` command because it handles copying and
            // setting permissions (chmod) in a single, atomic-like operation
            // that is guaranteed to produce an executable binary.
            let install_cmd = format!(
                "install -m 755 {} {}",
                built_bin.to_string_lossy(),
                current_exe.to_string_lossy()
            );

            if std::process::Command::new("sh")
                .arg("-c")
                .arg(&install_cmd)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
            {
                // Ensure permissions are 755 and restore ownership if running via sudo.
                // This ensures the binary is usable by the user even if 'install'
                // behavior varied or if the process is still owned by root.
                let path_str = format!("\"{}\"", current_exe.to_string_lossy());
                let chown_part = if let Ok(user) = std::env::var("SUDO_USER") {
                    format!("chown {} {}", user, path_str)
                } else {
                    "true".to_string()
                };

                let finalize_cmd = format!("sleep 1 && chmod 755 {} && {}", path_str, chown_part);

                if std::process::Command::new("sh")
                    .arg("-c")
                    .arg(&finalize_cmd)
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    success("pacwoman updated and permissions finalized", cfg);
                } else {
                    warn(
                        "pacwoman updated, but failed to finalize permissions/ownership",
                        cfg,
                    );
                }
            } else {
                error("failed to install new binary via 'install' command", cfg);
            }
        } else {
            error("failed to build pacwoman from source", cfg);
        }
    } else {
        error("failed to clone pacwoman repository", cfg);
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(tmp_dir);
}

fn check_root(cfg: &config::ResolvedConfig) {
    if unsafe { libc::getuid() } != 0 {
        error("this operation requires root privileges", cfg);
        process::exit(1);
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn read_targets_from_stdin() -> Vec<String> {
    let mut targets = Vec::new();
    let stdin = std::io::stdin();
    let mut lines = stdin.lines();
    while let Some(Ok(line)) = lines.next() {
        for target in line.split_whitespace() {
            targets.push(target.to_string());
        }
    }
    targets
}

fn main() {
    let mut cli = match Cli::parse() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("  ERROR: {e}");
            process::exit(1);
        }
    };

    if cli.targets.is_empty() {
        match cli.op {
            Op::Sync | Op::Remove | Op::Upgrade => {
                if cli.sysupgrade == 0 {
                    cli.targets = read_targets_from_stdin();
                }
            }
            _ => {}
        }
    }

    if cli.version {
        print_version();
        process::exit(2);
    }

    if cli.help {
        print_help(&cli.op);
        process::exit(2);
    }

    match cli.op {
        Op::CheckConfig => {
            let ok = config::Config::check();
            process::exit(if ok { 0 } else { 1 });
        }
        Op::GenConfig => {
            let (cfg, _, _) = config::Config::load();
            let mut cfg = cfg;
            cfg.plain = cli.plain;
            cfg.noprogressbar = cli.noprogressbar;

            match config::Config::write_default() {
                Ok(path) => {
                    success(&format!("wrote default config to {}", path.display()), &cfg);
                    process::exit(0);
                }
                Err(e) => {
                    error(&format!("could not write config: {e}"), &cfg);
                    process::exit(1);
                }
            }
        }
        _ => {}
    }

    let (cfg, parse_errors, colour_errors) = config::Config::load();

    let mut cfg = cfg;
    cfg.plain = cli.plain;
    cfg.noprogressbar = cli.noprogressbar;

    for e in &parse_errors {
        warn(&format!("config: {e}"), &cfg);
    }
    for e in &colour_errors {
        warn(&format!("config: {e} (using Mocha default)"), &cfg);
    }

    let _is_read_only = match cli.op {
        Op::Sync => cli.s_info || cli.s_search || cli.s_list || cli.s_groups,
        _ => false,
    };

    if let Some(ver) = check_for_update() {
        if cli.sysupgrade > 0 {
            perform_self_update(&cfg);
        } else {
            info(
                &format!(
                    "a new version of pacwoman is available ({}), run with -u to update",
                    ver
                ),
                &cfg,
            );
        }
    }
    handle_stale_lock(&cfg);

    let interrupted = Arc::new(AtomicBool::new(false));
    {
        let flag = Arc::clone(&interrupted);
        ctrlc::set_handler(move || {
            flag.store(true, Ordering::SeqCst);
        })
        .expect("could not set Ctrl-C handler");
    }

    // ── Panic Hook ──────────────────────────────────────────────────────────────────
    std::panic::set_hook(Box::new(|info| {
        eprintln!("critical error occurred: {}", info);
        // We can't easily access the Alpm handle here as it's in main's scope.
        // However, Alpm's Drop implementation handles trans_release() and lock removal.
        // The panic will trigger the unwind and drop the handle.
    }));

    let (mut handle, cfg) = make_handle(&cli, cfg);

    if cli.deptest {
        do_deptest(&handle, &cli, &cfg);
    }

    match cli.op {
        Op::Sync => do_sync(&mut handle, &cli, &interrupted, &cfg),
        Op::Remove => do_remove(&mut handle, &cli, &interrupted, &cfg),
        Op::Upgrade => do_upgrade(&mut handle, &cli, &interrupted, &cfg),
        Op::Query => do_query(&handle, &cli, &cfg),
        Op::Database => do_database(&handle, &cli, &interrupted, &cfg),
        Op::Files => {
            error(
                "files database search is currently unavailable (symbol not found in libalpm)",
                &cfg,
            );
            return;
        }
        Op::Declarative => do_declarative(&mut handle, &cli, &interrupted, &cfg),
        Op::None => {
            error("no operation specified (try -S, -R, -Q, -U, -D, -F)", &cfg);
            process::exit(1);
        }
        Op::CheckConfig | Op::GenConfig => unreachable!(),
    }
}
