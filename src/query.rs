use alpm::{Alpm, PackageReason};
use crate::render::*;

pub struct QueryOpts {
    pub info:       bool,
    pub deps:       bool,
    pub explicit:   bool,
    pub unrequired: bool,
    pub upgrades:   bool,
    pub quiet:      bool,
}

pub fn query(handle: &Alpm, pkgs: &[String], opts: &QueryOpts) {
    let db = handle.localdb();

    let targets: Vec<_> = if pkgs.is_empty() {
        db.pkgs().iter().collect()
    } else {
        pkgs.iter()
            .filter_map(|n| {
                db.pkg(n.as_str()).map_err(|_| error(&format!("package not found: {n}"))).ok()
            })
            .collect()
    };

    for pkg in targets {
        if opts.upgrades {
            // handle upgrades separately — needs syncdbs
            for syncdb in handle.syncdbs() {
                if let Ok(sync_pkg) = syncdb.pkg(pkg.name()) {
                    if alpm::vercmp(sync_pkg.version().as_str(), pkg.version().as_str())
                        == std::cmp::Ordering::Greater
                    {
                        if opts.quiet {
                            println!("{}", pkg.name());
                        } else {
                            println!(
                                "  {TEXT}{}{RST}  {DIM}{}{RST} {SURFACE2}→{RST} {GREEN}{}{RST}",
                                pkg.name(), pkg.version(), sync_pkg.version()
                            );
                        }
                    }
                }
            }
            continue;
        }

        if opts.deps && pkg.reason() != PackageReason::Depend       { continue; }
        if opts.explicit && pkg.reason() != PackageReason::Explicit { continue; }
        if opts.unrequired && !pkg.required_by().is_empty()         { continue; }

        if opts.info {
            print_pkg_info(pkg);
        } else {
            print_pkg_line(pkg, opts.quiet);
        }
    }
}

pub fn query_owns(handle: &Alpm, file: &str) {
    // normalise: strip leading slash for comparison
    let needle = file.trim_start_matches('/');
    let mut found = false;

    for pkg in handle.localdb().pkgs() {
        for f in pkg.files().files() {
            let name = String::from_utf8_lossy(f.name());
            if name.trim_start_matches('/') == needle {
                println!(
                    "  {TEXT}{}{RST} is owned by {GREEN}{} {}{RST}",
                    file, pkg.name(), pkg.version()
                );
                found = true;
                break; // one owner is enough per package
            }
        }
    }
    if !found {
        error(&format!("no package owns {file}"));
    }
}

pub fn query_search(handle: &Alpm, terms: &[String]) {
    if terms.is_empty() {
        warn("no search terms given");
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
            println!(
                "  {GREEN}{}{RST} {DIM}{}{RST}\n    {SUBTEXT1}{}{RST}",
                pkg.name(),
                pkg.version(),
                pkg.desc().unwrap_or(""),
            );
            any = true;
        }
    }
    if !any {
        info("no matching packages found");
    }
}

fn print_pkg_line(pkg: &alpm::Package, quiet: bool) {
    if quiet {
        println!("{}", pkg.name());
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

fn print_pkg_info(pkg: &alpm::Package) {
    let reason_str = match pkg.reason() {
        PackageReason::Explicit => format!("{GREEN}explicit{RST}"),
        PackageReason::Depend   => format!("{YELLOW}dependency{RST}"),
    };

    let deps:  Vec<String> = pkg.depends().iter().map(|d| d.to_string()).collect();
    let opt:   Vec<String> = pkg.optdepends().iter().map(|d| d.to_string()).collect();
    let req:   Vec<String> = pkg.required_by().iter().map(|s| s.to_string()).collect();
    let files: Vec<String> = pkg.files().files().iter()
        .map(|f| String::from_utf8_lossy(f.name()).to_string())
        .collect();

    println!();
    kv("Name",           pkg.name());
    kv("Version",        pkg.version().as_str());
    kv("Description",    pkg.desc().unwrap_or("—"));
    kv("Architecture",   pkg.arch().unwrap_or("—"));
    kv("URL",            pkg.url().unwrap_or("—"));
    kv("Licenses",       &pkg.licenses().iter().map(|s| s.to_string()).collect::<Vec<_>>().join("  "));
    kv("Groups",         &pkg.groups().iter().map(|s| s.to_string()).collect::<Vec<_>>().join("  "));
    kv("Depends On",     &if deps.is_empty() { "—".into() } else { deps.join("  ") });
    kv("Optional Deps",  &if opt.is_empty()  { "—".into() } else { opt.join("\n                    ") });
    kv("Required By",    &if req.is_empty()  { "—".into() } else { req.join("  ") });
    kv("Install Reason", &reason_str);
    kv("Install Date",   &pkg.install_date().map(|d| d.to_string()).unwrap_or_else(|| "—".into()));
    kv("Install Size",   &human_size(pkg.isize()));
    kv("Packager",       pkg.packager().unwrap_or("—"));

    if !files.is_empty() {
        let shown = &files[..5.min(files.len())];
        kv("Files", &shown.join("\n                    "));
        if files.len() > 5 {
            println!("                    {DIM}… and {} more{RST}", files.len() - 5);
        }
    }
    println!();
}

fn kv(key: &str, val: &str) {
    println!("  {MAUVE}{BOLD}{key:<18}{RST} {TEXT}{val}{RST}");
}
