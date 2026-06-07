use alpm::{Alpm, PackageReason};
use crate::config::ResolvedConfig;
use crate::render::*;

pub struct QueryOpts {
    pub info:       bool,
    pub deps:       bool,
    pub explicit:   bool,
    pub unrequired: bool,
    pub upgrades:   bool,
    pub file_check: bool,
    pub foreign:    bool,
    pub native:     bool,
    pub list:       bool,
    pub groups:     bool,
    pub changelog:  bool,
    pub quiet:      bool,
}

pub fn query(handle: &Alpm, pkgs: &[String], opts: &QueryOpts, cfg: &ResolvedConfig) {
    let db = handle.localdb();

    let targets: Vec<_> = if pkgs.is_empty() {
        db.pkgs().iter().collect()
    } else {
        pkgs.iter()
            .filter_map(|n| {
                db.pkg(n.as_str()).map_err(|_| error(&format!("package not found: {n}"), cfg)).ok()
            })
            .collect()
    };

    for pkg in targets {
        if opts.file_check {
            let mut missing = Vec::new();
            for f in pkg.files().files() {
                let name = String::from_utf8_lossy(f.name());
                let path = std::path::Path::new(name.as_ref());
                if !path.exists() {
                    missing.push(path.to_string_lossy().to_string());
                }
            }
            if !missing.is_empty() {
                if opts.quiet {
                    println!("{}", pkg.name());
                } else {
                    if cfg.plain {
                        println!("  ✗ {}: {} files missing", pkg.name(), missing.len());
                        for m in missing {
                            println!("    missing: {m}");
                        }
                    } else {
                        println!("  {RED}✗{RST} {TEXT}{:<30}{RST} {} files missing", pkg.name(), missing.len());
                        for m in missing {
                            println!("    {DIM}missing: {m}{RST}");
                        }
                    }
                }
            } else if !opts.quiet {
                if cfg.plain {
                    println!("  ✓ {}: all files present", pkg.name());
                } else {
                    println!("  {GREEN}✓{RST} {TEXT}{:<30}{RST} all files present", pkg.name());
                }
            }
            continue;
        }

        if opts.upgrades {
            for syncdb in handle.syncdbs() {
                if let Ok(sync_pkg) = syncdb.pkg(pkg.name()) {
                    if alpm::vercmp(sync_pkg.version().as_str(), pkg.version().as_str())
                        == std::cmp::Ordering::Greater
                    {
                        if opts.quiet {
                            println!("{}", pkg.name());
                        } else {
                            if cfg.plain {
                                println!("  {} {} {} {}", pkg.name(), pkg.version(), "→", sync_pkg.version());
                            } else {
                                println!(
                                    "  {TEXT}{}{RST}  {DIM}{}{RST} {SURFACE2}→{RST} {GREEN}{}{RST}",
                                    pkg.name(), pkg.version(), sync_pkg.version()
                                );
                            }
                        }
                    }
                }
            }
            continue;
        }

        if opts.foreign {
            let is_foreign = handle.syncdbs().iter().all(|db| db.pkg(pkg.name()).is_err());
            if is_foreign {
                if opts.quiet {
                    println!("{}", pkg.name());
                } else {
                    if cfg.plain {
                        println!("  {:<30} {}", pkg.name(), pkg.version());
                    } else {
                        println!("  {TEXT}{:<30}{RST} {DIM}{}{RST}", pkg.name(), pkg.version());
                    }
                }
            }
            continue;
        }

        if opts.native {
            let is_native = handle.syncdbs().iter().any(|db| db.pkg(pkg.name()).is_ok());
            if is_native {
                if opts.quiet {
                    println!("{}", pkg.name());
                } else {
                    if cfg.plain {
                        println!("  {:<30} {}", pkg.name(), pkg.version());
                    } else {
                        println!("  {TEXT}{:<30}{RST} {DIM}{}{RST}", pkg.name(), pkg.version());
                    }
                }
            }
            continue;
        }

        if opts.list {
            if !opts.quiet {
                if cfg.plain {
                    println!("{}: {}", pkg.name(), pkg.version());
                } else {
                    println!("{MAUVE}{BOLD}{}: {}{RST}", pkg.name(), pkg.version());
                }
            }
            for f in pkg.files().files() {
                if cfg.plain {
                    println!("  {}", String::from_utf8_lossy(f.name()));
                } else {
                    println!("  {TEXT}{}{RST}", String::from_utf8_lossy(f.name()));
                }
            }
            continue;
        }

        if opts.groups {
            if !opts.quiet {
                if cfg.plain {
                    println!("{}: {}", pkg.name(), pkg.version());
                } else {
                    println!("{MAUVE}{BOLD}{}: {}{RST}", pkg.name(), pkg.version());
                }
            }
            for g in pkg.groups() {
                if cfg.plain {
                    println!("  {}", g);
                } else {
                    println!("  {TEXT}{}{RST}", g);
                }
            }
            continue;
        }

        if opts.changelog {
            if let Ok(cl) = pkg.changelog() {
                if cfg.plain {
                    println!("Changelog for {}: {}\n", pkg.name(), pkg.version());
                } else {
                    println!("{MAUVE}{BOLD}Changelog for {}: {}{RST}", pkg.name(), pkg.version());
                }
                println!("{:?}\n", cl);
            } else {
                warn(&format!("no changelog available for {}", pkg.name()), cfg);
            }
            continue;
        }

        if opts.deps && pkg.reason() != PackageReason::Depend       { continue; }
        if opts.explicit && pkg.reason() != PackageReason::Explicit { continue; }
        if opts.unrequired && !pkg.required_by().is_empty()         { continue; }

        if opts.info {
            print_pkg_info(pkg, cfg);
        } else {
            print_pkg_line(pkg, opts.quiet, cfg);
        }
    }
}

pub fn query_owns(handle: &Alpm, file: &str, cfg: &ResolvedConfig) {
    // normalise: strip leading slash for comparison
    let needle = file.trim_start_matches('/');
    let mut found = false;

    for pkg in handle.localdb().pkgs() {
        for f in pkg.files().files() {
            let name = String::from_utf8_lossy(f.name());
            if name.trim_start_matches('/') == needle {
                if cfg.plain {
                    println!("{} is owned by {} {}", file, pkg.name(), pkg.version());
                } else {
                    println!(
                        "  {TEXT}{}{RST} is owned by {GREEN}{} {}{RST}",
                        file, pkg.name(), pkg.version()
                    );
                }
                found = true;
                break; // one owner is enough per package
            }
        }
    }
    if !found {
        error(&format!("no package owns {file}"), cfg);
    }
}

pub fn query_search(handle: &Alpm, terms: &[String], cfg: &ResolvedConfig) {
    if terms.is_empty() {
        warn("no search terms given", cfg);
        return;
    }
    let db = handle.localdb();
    let mut any = false;
    for pkg in db.pkgs() {
        let name = pkg.name().to_lowercase();
        let desc = pkg.desc().unwrap_or("").to_lowercase();
        if terms.iter().all(|t| {
            let t = t.to_lowercase();
            name.contains(t.as_str()) || desc.contains(t.as_str())
        }) {
            if cfg.plain {
                println!("{} {}\n    {}", pkg.name(), pkg.version(), pkg.desc().unwrap_or(""));
            } else {
                println!(
                    "  {GREEN}{}{RST} {DIM}{}{RST}\n    {SUBTEXT1}{}{RST}",
                    pkg.name(),
                    pkg.version(),
                    pkg.desc().unwrap_or(""),
                );
            }
            any = true;
        }
    }
    if !any {
        info("no matching packages found", cfg);
    }
}

fn print_pkg_line(pkg: &alpm::Package, quiet: bool, cfg: &ResolvedConfig) {
    if quiet {
        println!("{}", pkg.name());
    } else {
        if cfg.plain {
            println!("  • {:<30} {}", pkg.name(), pkg.version());
        } else {
            let reason_col = match pkg.reason() {
                PackageReason::Explicit => GREEN,
                PackageReason::Depend   => SUBTEXT1,
            };
            println!(
                "  {reason_col}•{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}",
                pkg.name(), pkg.version()
            );
        }
    }
}

fn print_pkg_info(pkg: &alpm::Package, cfg: &ResolvedConfig) {
    let reason_str = match pkg.reason() {
        PackageReason::Explicit => if cfg.plain { "explicit".into() } else { format!("{GREEN}explicit{RST}") },
        PackageReason::Depend   => if cfg.plain { "dependency".into() } else { format!("{YELLOW}dependency{RST}") },
    };

    let deps:  Vec<String> = pkg.depends().iter().map(|d| d.to_string()).collect();
    let opt:   Vec<String> = pkg.optdepends().iter().map(|d| d.to_string()).collect();
    let req:   Vec<String> = pkg.required_by().iter().map(|s| s.to_string()).collect();
    let files: Vec<String> = pkg.files().files().iter()
        .map(|f| String::from_utf8_lossy(f.name()).to_string())
        .collect();

    println!();
    kv("Name",           pkg.name(), cfg);
    kv("Version",        pkg.version().as_str(), cfg);
    kv("Description",    pkg.desc().unwrap_or("—"), cfg);
    kv("Architecture",   pkg.arch().unwrap_or("—"), cfg);
    kv("URL",            pkg.url().unwrap_or("—"), cfg);
    kv("Licenses",       &pkg.licenses().iter().map(|s| s.to_string()).collect::<Vec<_>>().join("  "), cfg);
    kv("Groups",         &pkg.groups().iter().map(|s| s.to_string()).collect::<Vec<_>>().join("  "), cfg);
    kv("Depends On",     &if deps.is_empty() { "—".into() } else { deps.join("  ") }, cfg);
    kv("Optional Deps",  &if opt.is_empty()  { "—".into() } else { opt.join("\n                    ") }, cfg);
    kv("Required By",    &if req.is_empty()  { "—".into() } else { req.join("  ") }, cfg);
    kv("Install Reason", &reason_str, cfg);
    kv("Install Date",   &pkg.install_date().map(|d| d.to_string()).unwrap_or_else(|| "—".into()), cfg);
    kv("Install Size",   &human_size(pkg.isize()), cfg);
    kv("Packager",       pkg.packager().unwrap_or("—"), cfg);

    if !files.is_empty() {
        let shown = &files[..5.min(files.len())];
        kv("Files", &shown.join("\n                    "), cfg);
        if files.len() > 5 {
            if cfg.plain {
                println!("                    … and {} more", files.len() - 5);
            } else {
                println!("                    {DIM}… and {} more{RST}", files.len() - 5);
            }
        }
    }
    println!();
}

fn kv(key: &str, val: &str, cfg: &ResolvedConfig) {
    if cfg.plain {
        println!("{key:<18}: {val}");
    } else {
        println!("  {MAUVE}{BOLD}{key:<18}{RST} {TEXT}{val}{RST}");
    }
}
