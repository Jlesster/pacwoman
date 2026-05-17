mod render;
mod config;
mod callbacks;
mod query;

use std::process;
use alpm::{Alpm, SigLevel, TransFlag, PackageReason};
use render::*;
use query::{QueryOpts, query, query_owns, query_search};

const ROOT:      &str = "/";
const DBPATH:    &str = "/var/lib/pacman";
const LOGFILE:   &str = "/var/log/pacman.log";
const GPGDIR:    &str = "/etc/pacman.d/gnupg";
const CACHEDIRS: &[&str] = &["/var/cache/pacman/pkg/"];

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct Config {
    op:           Op,
    targets:      Vec<String>,
    refresh:      u8,
    sysupgrade:   bool,
    downloadonly: bool,
    nosave:       bool,
    recursive:    u8,
    q_info:       bool,
    q_deps:       bool,
    q_explicit:   bool,
    q_unreq:      bool,
    q_upgrades:   bool,
    q_quiet:      bool,
    q_owns:       bool,
    q_search:     bool,
    noconfirm:    bool,
    asdeps:       bool,
    asexplicit:   bool,
}

#[derive(Default, Debug, PartialEq)]
enum Op { #[default] None, Sync, Remove, Upgrade, Query, Database }

impl Config {
    fn parse() -> Self {
        let mut cfg = Config::default();
        let args: Vec<String> = std::env::args().skip(1).collect();
        for arg in &args {
            if arg.starts_with("--") {
                match arg.as_str() {
                    "--sync"         => cfg.op = Op::Sync,
                    "--remove"       => cfg.op = Op::Remove,
                    "--upgrade"      => cfg.op = Op::Upgrade,
                    "--query"        => cfg.op = Op::Query,
                    "--database"     => cfg.op = Op::Database,
                    "--refresh"      => cfg.refresh += 1,
                    "--sysupgrade"   => cfg.sysupgrade = true,
                    "--downloadonly" => cfg.downloadonly = true,
                    "--nosave"       => cfg.nosave = true,
                    "--recursive"    => cfg.recursive += 1,
                    "--info"         => cfg.q_info = true,
                    "--deps"         => cfg.q_deps = true,
                    "--explicit"     => cfg.q_explicit = true,
                    "--unrequired"   => cfg.q_unreq = true,
                    "--upgrades"     => cfg.q_upgrades = true,
                    "--quiet"        => cfg.q_quiet = true,
                    "--owns"         => cfg.q_owns = true,
                    "--search"       => cfg.q_search = true,
                    "--noconfirm"    => cfg.noconfirm = true,
                    "--asdeps"       => cfg.asdeps = true,
                    "--asexplicit"   => cfg.asexplicit = true,
                    _                => {}
                }
            } else if arg.starts_with('-') {
                for c in arg.chars().skip(1) {
                    match c {
                        'S' => cfg.op = Op::Sync,
                        'R' => cfg.op = Op::Remove,
                        'U' => cfg.op = Op::Upgrade,
                        'Q' => cfg.op = Op::Query,
                        'D' => cfg.op = Op::Database,
                        'y' => cfg.refresh += 1,
                        'u' => cfg.sysupgrade = true,
                        'w' => cfg.downloadonly = true,
                        'n' => cfg.nosave = true,
                        's' => match cfg.op {
                            Op::Remove => cfg.recursive += 1,
                            _          => cfg.q_search = true,
                        },
                        'i' => cfg.q_info = true,
                        'd' => cfg.q_deps = true,
                        'e' => cfg.q_explicit = true,
                        't' => cfg.q_unreq = true,
                        'k' => cfg.q_upgrades = true,
                        'q' => cfg.q_quiet = true,
                        'o' => cfg.q_owns = true,
                        _   => {}
                    }
                }
            } else {
                cfg.targets.push(arg.clone());
            }
        }
        cfg
    }
}

// ── pacman.conf parser ────────────────────────────────────────────────────────

fn conf_value<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let rest = line.strip_prefix(key)?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('=')?;
    Some(rest.trim())
}

fn collect_servers(conf_lines: &[&str], start: usize, repo: &str) -> Vec<String> {
    let arch = std::env::consts::ARCH; // "x86_64"
    let mut servers = Vec::new();
    for line in conf_lines[start..].iter() {
        let line = line.trim();
        if line.starts_with('[') { break; }
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some(url) = conf_value(line, "Server") {
            let url = url.replace("$repo", repo).replace("$arch", arch);
            servers.push(url);
        } else if let Some(path) = conf_value(line, "Include") {
            if let Ok(contents) = std::fs::read_to_string(path) {
                for ml in contents.lines() {
                    let ml = ml.trim();
                    if ml.is_empty() || ml.starts_with('#') { continue; }
                    if let Some(url) = conf_value(ml, "Server") {
                        let url = url.replace("$repo", repo).replace("$arch", arch);
                        servers.push(url);
                    }
                }
            }
        }
    }
    servers
}

fn register_sync_dbs(handle: &mut Alpm) {
    let raw = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let lines: Vec<&str> = raw.lines().collect();
    let sig = SigLevel::USE_DEFAULT;
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if !line.starts_with('[') || !line.ends_with(']') { continue; }
        let name = &line[1..line.len() - 1];
        if name == "options" { continue; }
        let servers = collect_servers(&lines, i + 1, name);
        match handle.register_syncdb_mut(name, sig) {
            Ok(mut db) => {
                for s in &servers {
                    db.add_server(s.as_str()).ok();
                }
            }
            Err(e) => warn(&format!("could not register repo '{name}': {e}")),
        }
    }
}

// ── Handle ────────────────────────────────────────────────────────────────────

fn make_handle() -> Alpm {
    let mut handle = Alpm::new(ROOT, DBPATH).unwrap_or_else(|e| {
        error(&format!("failed to init alpm: {e}"));
        process::exit(1);
    });
    handle.set_logfile(LOGFILE).ok();
    handle.set_gpgdir(GPGDIR).ok();
    for d in CACHEDIRS { handle.add_cachedir(*d).ok(); }
    register_sync_dbs(&mut handle);
    let cfg = config::Config::load();
    handle.set_log_cb(cfg.clone(), callbacks::log_cb);
    handle.set_event_cb(cfg.clone(), callbacks::event_cb);
    handle.set_progress_cb(cfg.clone(), callbacks::progress_cb);
    handle.set_dl_cb(cfg.clone(), callbacks::dl_cb);
    handle.set_question_cb(cfg.clone(), callbacks::question_cb);
    handle
}

// ── Sync (-S) ─────────────────────────────────────────────────────────────────

fn do_sync(handle: &mut Alpm, cfg: &Config) {
    if cfg.refresh > 0 {
        header("synchronising package databases");
        let force = cfg.refresh > 1;
        // update() returns Err if any single mirror fails, even when another
        // mirror for that repo succeeded. This is normal on CachyOS (or any
        // setup with stale/geo-filtered mirrors). Treat it as non-fatal so we
        // don't abort when the db was actually fetched successfully.
        match handle.syncdbs_mut().update(force) {
            Ok(false) => info("all databases are up to date"),
            Ok(true)  => success("databases updated"),
            Err(e)    => warn(&format!("some mirrors failed ({}); continuing", e)),
        }
    }

    if cfg.targets.is_empty() && !cfg.sysupgrade { return; }

    let mut flags = TransFlag::NONE;
    if cfg.downloadonly { flags |= TransFlag::DOWNLOAD_ONLY; }

    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"));
        process::exit(1);
    });

    if cfg.sysupgrade {
        header("starting full system upgrade");
        handle.sync_sysupgrade(false).unwrap_or_else(|e| {
            error(&format!("sysupgrade failed: {e}"));
            process::exit(1);
        });
    }

    // first pass: validate all targets exist
    let mut missing = Vec::new();
    let pkg_names: Vec<String> = cfg.targets.iter().filter_map(|t| {
        match handle.syncdbs().find_satisfier(t.as_str()) {
            Some(p) => Some(p.name().to_string()),
            None    => { missing.push(t.clone()); None }
        }
    }).collect();

    if !missing.is_empty() {
        for m in &missing { error(&format!("target not found: {m}")); }
        handle.trans_release().ok();
        process::exit(1);
    }

    // second pass: add to transaction (re-resolve to get a fresh borrow)
    for name in &pkg_names {
        if let Some(p) = handle.syncdbs().find_satisfier(name.as_str()) {
            handle.trans_add_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not add {name}: {e}"));
                process::exit(1);
            });
        }
    }

    trans_prepare_or_die(handle);
    print_sync_summary(handle);

    if !cfg.noconfirm && !confirm("proceed with installation?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    trans_commit_or_die(handle);

    if cfg.asdeps || cfg.asexplicit {
        let reason = if cfg.asdeps { PackageReason::Depend } else { PackageReason::Explicit };
        let names: Vec<String> = handle.trans_add().iter().map(|p| p.name().to_string()).collect();
        handle.trans_release().ok();
        for name in names {
            if let Ok(pkg) = handle.localdb().pkg(name.as_str()) {
                pkg.set_reason(reason).ok();
            }
        }
    } else {
        handle.trans_release().ok();
    }

    success("transaction complete");
}

// ── Remove (-R) ───────────────────────────────────────────────────────────────

fn do_remove(handle: &mut Alpm, cfg: &Config) {
    let mut flags = TransFlag::NONE;
    if cfg.nosave        { flags |= TransFlag::NO_SAVE; }
    if cfg.recursive > 0 { flags |= TransFlag::RECURSE; }

    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"));
        process::exit(1);
    });

    let mut missing = Vec::new();
    for name in &cfg.targets {
        if handle.localdb().pkg(name.as_str()).is_err() {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        for m in &missing { error(&format!("target not found: {m}")); }
        handle.trans_release().ok();
        process::exit(1);
    }

    for name in &cfg.targets {
        let pkg = handle.localdb().pkg(name.as_str()).unwrap();
        handle.trans_remove_pkg(pkg).unwrap_or_else(|e| {
            error(&format!("could not queue {name} for removal: {e}"));
            process::exit(1);
        });
    }

    trans_prepare_or_die(handle);
    print_remove_summary(handle);

    if !cfg.noconfirm && !confirm("proceed with removal?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    trans_commit_or_die(handle);
    handle.trans_release().ok();
    success("removal complete");
}

// ── Upgrade (-U) ──────────────────────────────────────────────────────────────

fn do_upgrade(handle: &mut Alpm, cfg: &Config) {
    handle.trans_init(TransFlag::NONE).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"));
        process::exit(1);
    });

    for path in &cfg.targets {
        match handle.pkg_load(path.as_str(), true, SigLevel::USE_DEFAULT) {
            Ok(pkg) => {
                handle.trans_add_pkg(pkg).unwrap_or_else(|e| {
                    error(&format!("could not add {path}: {e}"));
                    process::exit(1);
                });
            }
            Err(e) => {
                error(&format!("failed to load {path}: {e}"));
                process::exit(1);
            }
        }
    }

    trans_prepare_or_die(handle);
    print_sync_summary(handle);

    if !cfg.noconfirm && !confirm("proceed with installation?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    trans_commit_or_die(handle);
    handle.trans_release().ok();
    success("upgrade complete");
}

// ── Database (-D) ─────────────────────────────────────────────────────────────

fn do_database(handle: &Alpm, cfg: &Config) {
    let reason = if cfg.asdeps {
        Some(PackageReason::Depend)
    } else if cfg.asexplicit {
        Some(PackageReason::Explicit)
    } else {
        None
    };

    if let Some(r) = reason {
        for name in &cfg.targets {
            match handle.localdb().pkg(name.as_str()) {
                Ok(pkg) => {
                    pkg.set_reason(r).unwrap_or_else(|e| {
                        error(&format!("could not set reason for {name}: {e}"));
                    });
                    let label = if r == PackageReason::Depend { "dependency" } else { "explicit" };
                    info(&format!("{name}: install reason set to '{label}'"));
                }
                Err(_) => error(&format!("package not found: {name}")),
            }
        }
    } else {
        warn("no --asdeps or --asexplicit flag given; nothing to do");
    }
}

// ── Query (-Q) ────────────────────────────────────────────────────────────────

fn do_query(handle: &Alpm, cfg: &Config) {
    if cfg.q_owns && !cfg.targets.is_empty() {
        for t in &cfg.targets { query_owns(handle, t); }
        return;
    }
    if cfg.q_search {
        query_search(handle, &cfg.targets);
        return;
    }
    let opts = QueryOpts {
        info:       cfg.q_info,
        deps:       cfg.q_deps,
        explicit:   cfg.q_explicit,
        unrequired: cfg.q_unreq,
        upgrades:   cfg.q_upgrades,
        quiet:      cfg.q_quiet,
    };
    query(handle, &cfg.targets, &opts);
}

// ── Transaction helpers ───────────────────────────────────────────────────────

fn trans_prepare_or_die(handle: &mut Alpm) {
    if let Err(e) = handle.trans_prepare().map_err(|e| e.to_string()) {
        handle.trans_release().ok();
        error(&format!("prepare failed: {e}"));
        process::exit(1);
    }
}

fn trans_commit_or_die(handle: &mut Alpm) {
    if let Err(e) = handle.trans_commit().map_err(|e| e.to_string()) {
        handle.trans_release().ok();
        error(&format!("commit failed: {e}"));
        process::exit(1);
    }
}

// ── Summaries ─────────────────────────────────────────────────────────────────

fn print_sync_summary(handle: &Alpm) {
    let add = handle.trans_add();
    let rem = handle.trans_remove();

    if !add.is_empty() {
        println!("\n  {MAUVE}{BOLD}packages ({}):{RST}", add.len());
        let mut dl_total:   i64 = 0;
        let mut inst_total: i64 = 0;
        let mut net_change: i64 = 0;

        for p in add.iter() {
            let local   = handle.localdb().pkg(p.name());
            let is_up   = local.is_ok();
            let col     = if is_up { BLUE } else { GREEN };
            let sym     = if is_up { "⟳" } else { "↑" };
            let ver_str = if let Ok(ref l) = local {
                format!(" {DIM}{} → {}{RST}", l.version(), p.version())
            } else {
                format!(" {DIM}{}{RST}", p.version())
            };
            let old_sz = local.as_ref().map(|l| l.isize()).unwrap_or(0);
            println!("    {col}{sym}{RST} {TEXT}{:<30}{RST}{ver_str}  {SUBTEXT1}{}{RST}",
                p.name(), human_size(p.isize()));
            dl_total   += p.download_size();
            inst_total += p.isize();
            net_change += p.isize() - old_sz;
        }

        println!();
        println!("  {ROSEWATER}total download size:   {}{RST}", human_size(dl_total));
        println!("  {ROSEWATER}total installed size:  {}{RST}", human_size(inst_total));
        if net_change != inst_total {
            println!("  {ROSEWATER}net upgrade size:      {}{RST}", human_size(net_change));
        }
    }

    if !rem.is_empty() {
        println!("\n  {RED}{BOLD}removing ({}):{RST}", rem.len());
        for p in rem.iter() {
            println!("    {RED}✕{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}", p.name(), p.version());
        }
    }

    println!();
}

fn print_remove_summary(handle: &Alpm) {
    let rem = handle.trans_remove();
    if rem.is_empty() {
        warn("nothing to remove");
        return;
    }
    println!("\n  {RED}{BOLD}removing ({}):{RST}", rem.len());
    for p in rem.iter() {
        println!("    {RED}✕{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}", p.name(), p.version());
    }
    let freed: i64 = rem.iter().map(|p| p.isize()).sum();
    println!("\n  {ROSEWATER}freed space: {}{RST}", human_size(freed));
    println!();
}

// ── Misc helpers ──────────────────────────────────────────────────────────────

fn confirm(msg: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("\n  {MAUVE}{BOLD}::{RST} {TEXT}{msg} {SUBTEXT1}{hint}{RST} ");
    let _ = std::io::Write::flush(&mut std::io::stdout());
    let mut s = String::new();
    std::io::stdin().read_line(&mut s).ok();
    let t = s.trim().to_lowercase();
    if t.is_empty() { default_yes } else { t == "y" || t == "yes" }
}

fn check_root() {
    if unsafe { libc::geteuid() } != 0 {
        error("this operation requires root");
        process::exit(1);
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let cfg = Config::parse();
    if matches!(cfg.op, Op::Sync | Op::Remove | Op::Upgrade | Op::Database) {
        check_root();
    }
    let mut handle = make_handle();
    match cfg.op {
        Op::Sync     => do_sync(&mut handle, &cfg),
        Op::Remove   => do_remove(&mut handle, &cfg),
        Op::Upgrade  => do_upgrade(&mut handle, &cfg),
        Op::Query    => do_query(&handle, &cfg),
        Op::Database => do_database(&handle, &cfg),
        Op::None     => {
            error("no operation specified (try -S, -R, -Q, -U, -D)");
            process::exit(1);
        }
    }
}
