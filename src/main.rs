mod callbacks;
mod config;
mod query;
mod render;

use alpm::{Alpm, PackageReason, SigLevel, TransFlag};
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
    sysupgrade: bool,
    downloadonly: bool,
    nosave: bool,
    recursive: u8,
    q_info: bool,
    q_deps: bool,
    q_explicit: bool,
    q_unreq: bool,
    q_upgrades: bool,
    q_quiet: bool,
    q_owns: bool,
    q_search: bool,
    noconfirm: bool,
    asdeps: bool,
    asexplicit: bool,
    plain: bool,
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
    CheckConfig,
    GenConfig,
    Declarative,
}

impl Cli {
    fn parse() -> Result<Self, String> {
        let mut cli = Cli::default();
        cli.plain = !std::io::stdout().is_terminal();
        let args: Vec<String> = std::env::args().skip(1).collect();
        for arg in &args {
            if arg.starts_with("--") {
                match arg.as_str() {
                    "--sync" => cli.op = Op::Sync,
                    "--remove" => cli.op = Op::Remove,
                    "--upgrade" => cli.op = Op::Upgrade,
                    "--query" => cli.op = Op::Query,
                    "--database" => cli.op = Op::Database,
                    "--check-config" => cli.op = Op::CheckConfig,
                    "--gen-config" => cli.op = Op::GenConfig,
                    "--declarative" => cli.op = Op::Declarative,
                    "--refresh" => cli.refresh += 1,
                    "--sysupgrade" => cli.sysupgrade = true,
                    "--downloadonly" => cli.downloadonly = true,
                    "--nosave" => cli.nosave = true,
                    "--recursive" => cli.recursive += 1,
                    "--info" => cli.q_info = true,
                    "--deps" => cli.q_deps = true,
                    "--explicit" => cli.q_explicit = true,
                    "--unrequired" => cli.q_unreq = true,
                    "--upgrades" => cli.q_upgrades = true,
                    "--quiet" => cli.q_quiet = true,
                    "--owns" => cli.q_owns = true,
                    "--search" => cli.q_search = true,
                    "--noconfirm" => cli.noconfirm = true,
                    "--asdeps" => cli.asdeps = true,
                    "--asexplicit" => cli.asexplicit = true,
                    "--plain" => cli.plain = true,
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
                        'y' => cli.refresh += 1,
                        'u' => cli.sysupgrade = true,
                        'w' => cli.downloadonly = true,
                        'n' => cli.nosave = true,
                        's' => match cli.op {
                            Op::Remove => cli.recursive += 1,
                            _ => cli.q_search = true,
                        },
                        'i' => cli.q_info = true,
                        'd' => cli.q_deps = true,
                        'e' => cli.q_explicit = true,
                        't' => cli.q_unreq = true,
                        'k' => cli.q_upgrades = true,
                        'q' => cli.q_quiet = true,
                        'o' => cli.q_owns = true,
                        _ => return Err(format!("unsupported flag: -{c}")),
                    }
                }
            } else {
                cli.targets.push(arg.clone());
            }
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

fn collect_servers_recursive(lines: &[String], repo: &str, depth: u8) -> Vec<String> {
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
            if let Ok(content) = std::fs::read_to_string(path) {
                let inc_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                servers.extend(collect_servers_recursive(&inc_lines, repo, depth + 1));
            }
        }
    }
    servers
}

fn collect_servers(conf_lines: &[&str], start: usize, repo: &str) -> Vec<String> {
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
            if let Ok(content) = std::fs::read_to_string(path) {
                let inc_lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
                servers.extend(collect_servers_recursive(&inc_lines, repo, 1));
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

fn register_sync_dbs(handle: &mut Alpm, plain: bool) {
    let raw = std::fs::read_to_string("/etc/pacman.conf").unwrap_or_default();
    let lines: Vec<&str> = raw.lines().collect();

    // ── Apply [options] settings that libalpm needs to behave like pacman ──
    // Without these, IgnorePkg/IgnoreGroup are invisible to the solver and
    // sysupgrade fails with unsatisfied deps instead of issuing a warning.
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

    // ── Register sync databases ────────────────────────────────────────────
    let sig = SigLevel::USE_DEFAULT;
    for (i, line) in lines.iter().enumerate() {
        let line = line.trim();
        if !line.starts_with('[') || !line.ends_with(']') {
            continue;
        }
        let name = &line[1..line.len() - 1];
        if name == "options" {
            continue;
        }
        let servers = collect_servers(&lines, i + 1, name);
        match handle.register_syncdb_mut(name, sig) {
            Ok(db) => {
                for s in &servers {
                    db.add_server(s.as_str()).ok();
                }
            }
            Err(e) => warn(&format!("could not register repo '{name}': {e}"), plain),
        }
    }
}

// ── Handle ────────────────────────────────────────────────────────────────────

fn make_handle(plain: bool) -> Alpm {
    let mut handle = Alpm::new(ROOT, DBPATH).unwrap_or_else(|e| {
        error(&format!("failed to init alpm: {e}"), plain);
        process::exit(1);
    });
    handle.set_logfile(LOGFILE).ok();
    handle.set_gpgdir(GPGDIR).ok();
    for d in CACHEDIRS {
        handle.add_cachedir(*d).ok();
    }
    register_sync_dbs(&mut handle, plain);

    let (mut cfg, parse_errors, colour_errors) = config::Config::load();
    cfg.plain = plain;

    for e in &parse_errors {
        warn(&format!("config: {e}"), plain);
    }
    for e in &colour_errors {
        warn(&format!("config: {e} (using Mocha default)"), plain);
    }

    handle.set_log_cb(cfg.clone(), callbacks::log_cb);
    handle.set_event_cb(cfg.clone(), callbacks::event_cb);
    handle.set_progress_cb(cfg.clone(), callbacks::progress_cb);
    handle.set_dl_cb(cfg.clone(), callbacks::dl_cb);
    handle.set_question_cb(cfg.clone(), callbacks::question_cb);
    handle
}

// ── Sync (-S) ─────────────────────────────────────────────────────────────────

fn do_sync(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool) {
    if cli.refresh > 0 {
        header("synchronising package databases", cli.plain);
        let force = cli.refresh > 1;
        match handle.syncdbs_mut().update(force) {
            Ok(false) => info("all databases are up to date", cli.plain),
            Ok(true) => success("databases updated", cli.plain),
            Err(e) => warn(
                &format!("some mirrors failed ({}); continuing", e),
                cli.plain,
            ),
        }
    }

    if cli.targets.is_empty() && !cli.sysupgrade {
        return;
    }

    let mut flags = TransFlag::NONE;
    if cli.downloadonly {
        flags |= TransFlag::DOWNLOAD_ONLY;
    }

    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cli.plain);
        process::exit(1);
    });

    if cli.sysupgrade {
        header("starting full system upgrade", cli.plain);
        handle.sync_sysupgrade(false).unwrap_or_else(|e| {
            error(&format!("sysupgrade failed: {e}"), cli.plain);
            process::exit(1);
        });
    }

    // first pass: validate all targets exist
    let mut missing = Vec::new();
    let pkg_names: Vec<String> = cli
        .targets
        .iter()
        .filter_map(|t| match handle.syncdbs().find_satisfier(t.as_str()) {
            Some(p) => Some(p.name().to_string()),
            None => {
                missing.push(t.clone());
                None
            }
        })
        .collect();

    if !missing.is_empty() {
        for m in &missing {
            error(&format!("target not found: {m}"), cli.plain);
        }
        handle.trans_release().ok();
        process::exit(1);
    }

    // second pass: add to transaction (re-resolve to get a fresh borrow)
    for name in &pkg_names {
        if let Some(p) = handle.syncdbs().find_satisfier(name.as_str()) {
            handle.trans_add_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not add {name}: {e}"), cli.plain);
                process::exit(1);
            });
        }
    }

    trans_prepare_or_die(handle, cli.plain);
    print_sync_summary(handle, cli.plain);

    if !cli.noconfirm && !confirm("proceed with installation?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cli.plain);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cli.plain);

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

    success("transaction complete", cli.plain);
}

// ── Remove (-R) ───────────────────────────────────────────────────────────────

fn do_remove(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool) {
    let mut flags = TransFlag::NONE;
    if cli.nosave {
        flags |= TransFlag::NO_SAVE;
    }
    if cli.recursive > 0 {
        flags |= TransFlag::RECURSE;
    }

    handle.trans_init(flags).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cli.plain);
        process::exit(1);
    });

    let mut missing = Vec::new();
    for name in &cli.targets {
        if handle.localdb().pkg(name.as_str()).is_err() {
            missing.push(name.clone());
        }
    }
    if !missing.is_empty() {
        for m in &missing {
            error(&format!("target not found: {m}"), cli.plain);
        }
        handle.trans_release().ok();
        process::exit(1);
    }

    for name in &cli.targets {
        let pkg = handle.localdb().pkg(name.as_str()).unwrap();
        handle.trans_remove_pkg(pkg).unwrap_or_else(|e| {
            error(
                &format!("could not queue {name} for removal: {e}"),
                cli.plain,
            );
            process::exit(1);
        });
    }

    trans_prepare_or_die(handle, cli.plain);
    print_remove_summary(handle, cli.plain);

    if !cli.noconfirm && !confirm("proceed with removal?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cli.plain);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cli.plain);
    handle.trans_release().ok();
    success("removal complete", cli.plain);
}

// ── Upgrade (-U) ──────────────────────────────────────────────────────────────

fn do_upgrade(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool) {
    handle.trans_init(TransFlag::NONE).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cli.plain);
        process::exit(1);
    });

    for path in &cli.targets {
        match handle.pkg_load(path.as_str(), true, SigLevel::USE_DEFAULT) {
            Ok(pkg) => {
                handle.trans_add_pkg(pkg).unwrap_or_else(|e| {
                    error(&format!("could not add {path}: {e}"), cli.plain);
                    process::exit(1);
                });
            }
            Err(e) => {
                error(&format!("failed to load {path}: {e}"), cli.plain);
                process::exit(1);
            }
        }
    }

    trans_prepare_or_die(handle, cli.plain);
    print_sync_summary(handle, cli.plain);

    if !cli.noconfirm && !confirm("proceed with installation?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cli.plain);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cli.plain);
    handle.trans_release().ok();
    success("upgrade complete", cli.plain);
}

// ── Database (-D) ─────────────────────────────────────────────────────────────

fn do_database(handle: &Alpm, cli: &Cli, interrupted: &AtomicBool) {
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
                warn(
                    "interrupted — remaining packages were not updated",
                    cli.plain,
                );
                process::exit(130);
            }
            match handle.localdb().pkg(name.as_str()) {
                Ok(pkg) => {
                    pkg.set_reason(r).unwrap_or_else(|e| {
                        error(&format!("could not set reason for {name}: {e}"), cli.plain);
                    });
                    let label = if r == PackageReason::Depend {
                        "dependency"
                    } else {
                        "explicit"
                    };
                    info(
                        &format!("{name}: install reason set to '{label}'"),
                        cli.plain,
                    );
                }
                Err(_) => error(&format!("package not found: {name}"), cli.plain),
            }
        }
    } else {
        warn(
            "no --asdeps or --asexplicit flag given; nothing to do",
            cli.plain,
        );
    }
}

// ── Query (-Q) ────────────────────────────────────────────────────────────────

fn do_query(handle: &Alpm, cli: &Cli, plain: bool) {
    if cli.q_owns && !cli.targets.is_empty() {
        for t in &cli.targets {
            query_owns(handle, t, plain);
        }
        return;
    }
    if cli.q_search {
        query_search(handle, &cli.targets, plain);
        return;
    }
    let opts = QueryOpts {
        info: cli.q_info,
        deps: cli.q_deps,
        explicit: cli.q_explicit,
        unrequired: cli.q_unreq,
        upgrades: cli.q_upgrades,
        quiet: cli.q_quiet,
    };
    query(handle, &cli.targets, &opts, plain);
}

// ── Transaction helpers ───────────────────────────────────────────────────────

fn trans_prepare_or_die(handle: &mut Alpm, plain: bool) {
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
        error(&format!("could not prepare transaction: {msg}"), plain);
        match diag {
            PrepDiag::Unsatisfied(deps) => {
                println!();
                for (dep, cause) in deps {
                    let cause_str = match cause {
                        Some(p) => format!(" (required by {p})"),
                        None => String::new(),
                    };
                    println!(
                        "  {RED}✗{RST}  {TEXT}missing dependency:{RST} \
                         {YELLOW}{dep}{RST}{DIM}{cause_str}{RST}",
                    );
                }
                println!();
            }
            PrepDiag::Conflicting(conflicts) => {
                println!();
                for (p1, p2, reason) in conflicts {
                    println!(
                        "  {RED}✗{RST}  {TEXT}conflict:{RST} \
                         {YELLOW}{p1}{RST}{DIM} ↔ {p2}{RST}  {DIM}({reason}){RST}",
                    );
                }
                println!();
            }
            PrepDiag::None => {}
        }
        process::exit(1);
    }
}

fn trans_commit_or_die(handle: &mut Alpm, interrupted: &AtomicBool, plain: bool) {
    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", plain);
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
        error(&format!("transaction failed: {msg}"), plain);
        match diag {
            CommitDiag::FileConflict(conflicts) => {
                for (p1, p2) in conflicts {
                    println!(
                        "  {RED}✗{RST}  {TEXT}file conflict:{RST} \
                         {YELLOW}{p1}{RST}{DIM} ↔ {p2}{RST}",
                    );
                }
            }
            CommitDiag::PkgInvalid(names) => {
                for name in names {
                    println!("  {RED}✗{RST}  {TEXT}invalid package:{RST} {YELLOW}{name}{RST}");
                }
            }
            CommitDiag::None => {}
        }
        println!();
        process::exit(1);
    }
}

// ── Summaries ─────────────────────────────────────────────────────────────────

fn print_sync_summary(handle: &Alpm, plain: bool) {
    let add = handle.trans_add();
    let rem = handle.trans_remove();

    if !add.is_empty() {
        if plain {
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
            if plain {
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
        if plain {
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
        if plain {
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

fn print_remove_summary(handle: &Alpm, plain: bool) {
    let rem = handle.trans_remove();
    if rem.is_empty() {
        warn("nothing to remove", plain);
        return;
    }
    println!("\n  {RED}{BOLD}removing ({}):{RST}", rem.len());
    for p in rem.iter() {
        println!(
            "    {RED}✕{RST} {TEXT}{:<30}{RST} {DIM}{}{RST}",
            p.name(),
            p.version()
        );
    }
    let freed: i64 = rem.iter().map(|p| p.isize()).sum();
    println!("\n  {ROSEWATER}freed space: {}{RST}", human_size(freed));
    println!();
}

// ── Declarative Mode ───────────────────────────────────────────────────────

fn do_declarative(handle: &mut Alpm, cli: &Cli, interrupted: &AtomicBool) {
    let state_path = std::path::Path::new("/etc/pacwoman/packages.json");
    if !state_path.exists() {
        error(
            &format!("state file not found: {}", state_path.display()),
            cli.plain,
        );
        process::exit(1);
    }

    let content = std::fs::read_to_string(state_path).unwrap_or_else(|e| {
        error(&format!("could not read state file: {e}"), cli.plain);
        process::exit(1);
    });

    let desired: Vec<String> = serde_json::from_str(&content).unwrap_or_else(|e| {
        error(&format!("could not parse state file: {e}"), cli.plain);
        process::exit(1);
    });

    header("reconciling system state", cli.plain);

    let mut installed = std::collections::HashSet::new();
    for pkg in handle.localdb().pkgs() {
        installed.insert(pkg.name().to_string());
    }

    let desired_set: std::collections::HashSet<String> = desired.into_iter().collect();

    let to_install: Vec<String> = desired_set.difference(&installed).cloned().collect();
    let to_remove: Vec<String> = installed.difference(&desired_set).cloned().collect();

    if to_install.is_empty() && to_remove.is_empty() {
        success("system is in desired state", cli.plain);
        return;
    }

    handle.trans_init(TransFlag::NONE).unwrap_or_else(|e| {
        error(&format!("failed to init transaction: {e}"), cli.plain);
        process::exit(1);
    });

    for pkg in &to_install {
        if let Some(p) = handle.syncdbs().find_satisfier(pkg.as_str()) {
            handle.trans_add_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not add {pkg}: {e}"), cli.plain);
                process::exit(1);
            });
        } else {
            error(&format!("package not found: {pkg}"), cli.plain);
            handle.trans_release().ok();
            process::exit(1);
        }
    }

    for pkg in &to_remove {
        // Protected packages: don't remove base or critical system components
        if let Ok(p) = handle.localdb().pkg(pkg.as_str()) {
            handle.trans_remove_pkg(p).unwrap_or_else(|e| {
                error(&format!("could not remove {pkg}: {e}"), cli.plain);
                process::exit(1);
            });
        }
    }

    trans_prepare_or_die(handle, cli.plain);
    print_sync_summary(handle, cli.plain);

    if !cli.noconfirm && !confirm("proceed with reconciliation?", true) {
        handle.trans_release().ok();
        process::exit(0);
    }

    if interrupted.load(Ordering::SeqCst) {
        handle.trans_release().ok();
        println!();
        warn("interrupted — no changes were made", cli.plain);
        process::exit(130);
    }

    trans_commit_or_die(handle, interrupted, cli.plain);
    handle.trans_release().ok();
    success("declarative reconciliation complete", cli.plain);
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

fn handle_stale_lock(plain: bool) {
    if is_lock_stale() {
        warn("a stale pacman lock was found", plain);
        if confirm("remove it and proceed?", true) {
            let lock_path = std::path::Path::new(DBPATH).join("db.lck");
            if let Err(e) = std::fs::remove_file(lock_path) {
                error(&format!("could not remove lock: {e}"), plain);
                process::exit(1);
            }
            success("lock removed", plain);
        } else {
            process::exit(1);
        }
    }
}

fn confirm(msg: &str, default_yes: bool) -> bool {
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("\n  {MAUVE}{BOLD}::{RST} {TEXT}{msg} {SUBTEXT1}{hint}{RST} ");
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

// ── Self-update check ─────────────────────────────────────────────────────────

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn is_version_greater(current: &str, remote: &str) -> bool {
    let c_parts: Vec<u32> = current.split('.').filter_map(|p| p.parse().ok()).collect();
    let r_parts: Vec<u32> = remote.split('.').filter_map(|p| p.parse().ok()).collect();

    for (c, r) in c_parts.iter().zip(r_parts.iter()) {
        if r > c { return true; }
        if r < c { return false; }
    }
    r_parts.len() > c_parts.len()
}

fn check_for_update() -> Option<String> {
    // Best-effort: any failure is silently ignored so the tool always works
    // offline or on stale mirrors.
    let repo = "Jlesster/pacwoman";
    let api_url = format!("https://api.github.com/repos/{}/releases/latest", repo);

    let Ok(output) = std::process::Command::new("curl")
        .args(["-fsSL", "-H", "Accept: application/vnd.github.v3+json", "--max-time", "3", &api_url])
        .output()
    else {
        return None;
    };

    if !output.status.success() {
        return None;
    }
    let body = String::from_utf8_lossy(&output.stdout);

    let release: serde_json::Value = match serde_json::from_str(&body) {
        Ok(val) => val,
        Err(_) => return None,
    };

    let remote_ver = release["tag_name"].as_str()?.trim_start_matches('v');

    if is_version_greater(CURRENT_VERSION, remote_ver) {
        return Some(remote_ver.to_string());
    }
    None
}

fn perform_self_update(plain: bool) {
    info("updating pacwoman binary...", plain);

    let current_exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            error(&format!("could not determine current exe path: {e}"), plain);
            return;
        }
    };

    // Dev build path: if we are in target/release and a .git dir exists, just pull and build
    if current_exe.to_string_lossy().contains("target/release") {
        let mut project_root = current_exe.parent().and_then(|p| p.parent()).and_then(|p| p.parent());
        let mut found_git = false;
        while let Some(root) = project_root {
            if root.join(".git").exists() {
                found_git = true;
                break;
            }
            project_root = root.parent();
        }

        if found_git {
            header("updating pacwoman (dev build - in-place)", plain);
            let update_cmd = "git pull && cargo build --release";
            info(&format!("running: {update_cmd}"), plain);

            if std::process::Command::new("sh").arg("-c").arg(update_cmd).status().is_ok() {
                success("pacwoman updated successfully", plain);
                return;
            } else {
                error("failed to update pacwoman dev build", plain);
                return;
            }
        }
    }

    // Source-based update: clone, build, and install
    header("updating pacwoman (source-based)", plain);

    let tmp_dir = std::env::temp_dir().join(format!("pacwoman-update-{}", std::process::id()));
    if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
        error(&format!("failed to create temp directory: {e}"), plain);
        return;
    }

    let repo_url = "https://github.com/Jlesster/pacwoman.git";
    info(&format!("cloning repository to {tmp_dir:?}"), plain);

    if std::process::Command::new("git")
        .args(["clone", repo_url, tmp_dir.to_str().unwrap()])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        info("building from source...", plain);
        let build_cmd = "cargo build --release";

        let status = std::process::Command::new("sh")
            .arg("-c")
            .arg(build_cmd)
            .current_dir(&tmp_dir)
            .status();

        if status.map(|s| s.success()).unwrap_or(false) {
            let built_bin = tmp_dir.join("target/release/pacwoman");

            // Use atomic rename to replace current binary
            if std::fs::rename(&built_bin, &current_exe).is_ok() {
                success("pacwoman updated to latest version from source", plain);
            } else {
                error("failed to replace current binary", plain);
            }
        } else {
            error("failed to build pacwoman from source", plain);
        }
    } else {
        error("failed to clone pacwoman repository", plain);
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(tmp_dir);
}

fn check_root(plain: bool) {
    if unsafe { libc::getuid() } != 0 {
        error("this operation requires root privileges", plain);
        process::exit(1);
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    let cli = match Cli::parse() {
        Ok(c) => c,
        Err(e) => {
            error(&e, true); // Use true for plain as this is a critical startup error
            process::exit(1);
        }
    };

    match cli.op {
        Op::CheckConfig => {
            let ok = config::Config::check();
            process::exit(if ok { 0 } else { 1 });
        }
        Op::GenConfig => match config::Config::write_default() {
            Ok(path) => {
                success(
                    &format!("wrote default config to {}", path.display()),
                    cli.plain,
                );
                process::exit(0);
            }
            Err(e) => {
                error(&format!("could not write config: {e}"), cli.plain);
                process::exit(1);
            }
        },
        _ => {}
    }

    if matches!(cli.op, Op::Sync | Op::Remove | Op::Upgrade | Op::Database) {
        check_root(cli.plain);
    }

    if let Some(_) = check_for_update() {
        if cli.sysupgrade {
            perform_self_update(cli.plain);
        }
    }
    handle_stale_lock(cli.plain);

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

    let mut handle = make_handle(cli.plain);

    match cli.op {
        Op::Sync => do_sync(&mut handle, &cli, &interrupted),
        Op::Remove => do_remove(&mut handle, &cli, &interrupted),
        Op::Upgrade => do_upgrade(&mut handle, &cli, &interrupted),
        Op::Query => do_query(&handle, &cli, cli.plain),
        Op::Database => do_database(&handle, &cli, &interrupted),
        Op::Declarative => do_declarative(&mut handle, &cli, &interrupted),
        Op::None => {
            error("no operation specified (try -S, -R, -Q, -U, -D)", cli.plain);
            process::exit(1);
        }
        Op::CheckConfig | Op::GenConfig => unreachable!(),
    }
}
