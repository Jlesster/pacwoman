use std::io::{self, Write};
use alpm::{
    AnyEvent, AnyQuestion, AnyDownloadEvent,
    LogLevel, Event, Progress, Question,
    DownloadEvent, PackageOperation,
};
use crate::config::Config;

// ── Log callback ──────────────────────────────────────────────────────────────

pub fn log_cb(level: LogLevel, msg: &str, cfg: &mut Config) {
    let msg = msg.trim_end_matches('\n');
    let c   = &cfg.colors;
    let s   = &cfg.symbols;
    match level {
        LogLevel::WARNING => {
            if cfg.suppress.mirror_warnings && msg.contains("too many errors from") { return; }
            if cfg.suppress.mirror_warnings && msg.contains("some mirrors failed")  { return; }
            eprintln!(
                "  {peach}{warn}{RST}  {peach}{msg}{RST}",
                peach = c.peach, warn = s.warn, RST = c.reset,
            );
        }
        LogLevel::ERROR => {
            if cfg.suppress.mirror_errors && msg.contains("failed retrieving file") { return; }
            eprintln!(
                "  {red}{err}{RST}  {red}{msg}{RST}",
                red = c.red, err = s.error, RST = c.reset,
            );
        }
        _ => {}
    }
}

// ── Event callback ────────────────────────────────────────────────────────────

pub fn event_cb(event: AnyEvent, cfg: &mut Config) {
    match event.event() {
        Event::CheckDepsStart      => status("checking dependencies", cfg),
        Event::CheckDepsDone       => erase_line(),
        Event::FileConflictsStart  => status("checking file conflicts", cfg),
        Event::FileConflictsDone   => erase_line(),
        Event::ResolveDepsStart    => status("resolving dependencies", cfg),
        Event::ResolveDepsDone     => erase_line(),
        Event::InterConflictsStart => status("checking inter-conflicts", cfg),
        Event::InterConflictsDone  => erase_line(),
        Event::TransactionStart    => {}
        Event::TransactionDone     => {}

        Event::PackageOperationStart(e) => {
            let c = &cfg.colors;
            let s = &cfg.symbols;
            let w = cfg.behavior.pkg_name_width;
            let (sym, col, name, extra) = match e.operation() {
                PackageOperation::Install(p) => (
                    &s.install, &c.green, p.name().to_string(),
                    format!("{dim}{}{RST}", p.version(), dim = c.dim, RST = c.reset),
                ),
                PackageOperation::Upgrade(new, old) => (
                    &s.upgrade, &c.blue, new.name().to_string(),
                    format!(
                        "{dim}{}{RST}{surf} → {RST}{dim}{}{RST}",
                        old.version(), new.version(),
                        dim = c.dim, RST = c.reset, surf = c.surface2,
                    ),
                ),
                PackageOperation::Downgrade(new, old) => (
                    &s.downgrade, &c.yellow, new.name().to_string(),
                    format!(
                        "{dim}{}{RST}{surf} → {RST}{dim}{}{RST}",
                        old.version(), new.version(),
                        dim = c.dim, RST = c.reset, surf = c.surface2,
                    ),
                ),
                PackageOperation::Reinstall(p, _) => (
                    &s.reinstall, &c.mauve, p.name().to_string(),
                    format!("{dim}{}{RST}", p.version(), dim = c.dim, RST = c.reset),
                ),
                PackageOperation::Remove(p) => (
                    &s.remove, &c.red, p.name().to_string(),
                    format!("{dim}{}{RST}", p.version(), dim = c.dim, RST = c.reset),
                ),
            };
            let name = trunc(&name, w).to_string();
            print!(
                "\r\x1b[2K  {col}{bold}{sym}{RST}  {text}{bold}{name:<w$}{RST}  {extra}",
                bold = c.bold, RST = c.reset, text = c.text,
            );
            let _ = io::stdout().flush();
        }
        Event::PackageOperationDone(_) => {}

        Event::IntegrityStart  => status("checking integrity", cfg),
        Event::IntegrityDone   => erase_line(),
        Event::LoadStart       => status("loading package files", cfg),
        Event::LoadDone        => erase_line(),

        Event::ScriptletInfo(e) => {
            if cfg.suppress.scriptlet { return; }
            let line = e.line().trim_end_matches('\n');
            if !line.is_empty() {
                let c = &cfg.colors;
                print!("\r\x1b[2K    {}{line}{RST}", c.surface2, RST = c.reset);
                let _ = io::stdout().flush();
            }
        }

        Event::DatabaseMissing(e) => {
            warn_msg(&format!("database file for '{}' is missing", e.dbname()), cfg);
        }

        Event::RetrieveStart  => {}
        Event::RetrieveDone   => {}
        Event::RetrieveFailed => {}

        Event::PkgRetrieveStart(e) => {
            let n = e.num();
            box_header(
                &format!("downloading {n} package{}", if n == 1 { "" } else { "s" }),
                cfg,
            );
        }
        Event::PkgRetrieveDone(_)   => {}
        Event::PkgRetrieveFailed(_) => {
            error_msg("failed to retrieve some packages", cfg);
        }

        Event::DiskSpaceStart  => status("checking disk space", cfg),
        Event::DiskSpaceDone   => erase_line(),

        Event::OptDepRemoval(e) => {
            if cfg.suppress.optdep_removal { return; }
            info_msg(&format!("optdep removed: {}", e.pkg().name()), cfg);
        }

        Event::HookStart(e) => {
            let when = match e.when() {
                alpm::HookWhen::PreTransaction  => "pre",
                alpm::HookWhen::PostTransaction => "post",
                _ => "?",
            };
            box_header(&format!("running {when}-transaction hooks"), cfg);
        }
        Event::HookDone(_) => {}

        Event::HookRunStart(e) => {
            if cfg.suppress.hook_names { return; }
            let c = &cfg.colors;
            let s = &cfg.symbols;
            print!(
                "\r\x1b[2K  {surf}{tick}{RST}  {sub}{}{RST}",
                e.name(),
                surf = c.surface2, tick = s.box_tick,
                sub  = c.subtext0, RST  = c.reset,
            );
            let _ = io::stdout().flush();
        }
        Event::HookRunDone(_) => {
            if !cfg.suppress.hook_names {
                print!("\r\x1b[2K");
                let _ = io::stdout().flush();
            }
        }

        Event::PacnewCreated(e) => {
            if cfg.suppress.pacnew { return; }
            info_msg(&format!("pacnew created: {}", e.file()), cfg);
        }
        Event::PacsaveCreated(e) => {
            if cfg.suppress.pacnew { return; }
            info_msg(&format!("pacsave created: {}", e.file()), cfg);
        }

        Event::KeyringStart     => status("loading keyring", cfg),
        Event::KeyringDone      => erase_line(),
        Event::KeyDownloadStart => {}
        Event::KeyDownloadDone  => {}

        _ => {}
    }
}

// ── Progress callback ─────────────────────────────────────────────────────────

pub fn progress_cb(
    prog: Progress, pkgname: &str, pct: i32,
    cur: usize, tot: usize, cfg: &mut Config,
) {
    let is_final = matches!(
        prog,
        Progress::AddStart | Progress::UpgradeStart | Progress::RemoveStart |
        Progress::DowngradeStart | Progress::ReinstallStart
    );
    if !is_final {
        if pct < 100 { status("processing", cfg); } else { erase_line(); }
        return;
    }

    let c   = &cfg.colors;
    let s   = &cfg.symbols;
    let col = match prog {
        Progress::AddStart | Progress::UpgradeStart         => cfg.bar.upgrade_color.resolve(c),
        Progress::RemoveStart                               => cfg.bar.remove_color.resolve(c),
        Progress::DowngradeStart | Progress::ReinstallStart => cfg.bar.downgrade_color.resolve(c),
        _                                                   => c.mauve.as_str(),
    };

    let bar_str = render_bar(pct as f64 / 100.0, col, cfg);

    let counter = if cfg.behavior.show_counter && tot > 1 {
        format!(
            "{surf}({cur:>2}/{tot:<2}){RST}  ",
            surf = c.surface2, RST = c.reset,
        )
    } else {
        String::new()
    };

    let w    = cfg.behavior.pkg_name_width;
    let name = trunc(pkgname, w);

    if pct >= 100 {
        print!(
            "\r\x1b[2K  {counter}{bar_str}  {col}{bold}{done}{RST}  {dim}{name}{RST}",
            bold = c.bold, dim = c.dim, RST = c.reset, done = s.done,
        );
    } else {
        print!(
            "\r\x1b[2K  {counter}{bar_str}  {sub}{pct:>3}%{RST}  {dim}{name}{RST}",
            sub = c.subtext1, dim = c.dim, RST = c.reset,
        );
        let _ = io::stdout().flush();
    }
}

// ── Download callback ─────────────────────────────────────────────────────────

pub fn dl_cb(filename: &str, event: AnyDownloadEvent, cfg: &mut Config) {
    match event.event() {
        DownloadEvent::Init(_) => {}

        DownloadEvent::Progress(e) => {
            let total  = e.total;
            let xfered = e.downloaded;
            if total == 0 { return; }

            let ratio = xfered as f64 / total as f64;
            let col   = cfg.bar.dl_color.resolve(&cfg.colors).to_string();
            let bar_s = render_bar(ratio, &col, cfg);
            let c     = &cfg.colors;
            let s     = &cfg.symbols;
            let w     = cfg.behavior.dl_name_width;
            let name  = trunc(strip_pkg_suffix(filename), w).to_string();

            print!(
                "\r\x1b[2K  {surf}{bar_sym}{RST}  {teal}{dl}{RST}  \
                 {text}{name:<w$}{RST}  {bar_s}  {sub}{xfer} / {tot}{RST}",
                surf    = c.surface2, bar_sym = s.box_bar,
                teal    = c.teal,     dl      = s.download,
                text    = c.text,     sub     = c.subtext1,
                RST     = c.reset,
                xfer    = human_size(xfered as i64),
                tot     = human_size(total as i64),
            );
            let _ = io::stdout().flush();
        }

        DownloadEvent::Completed(e) => {
            let c    = &cfg.colors;
            let s    = &cfg.symbols;
            let w    = cfg.behavior.dl_name_width;
            let name = trunc(strip_pkg_suffix(filename), w).to_string();
            if e.total > 0 {
                println!(
                    "\r\x1b[2K  {surf}{bar_sym}{RST}  {grn}{bold}{ok}{RST}  {text}{name}{RST}",
                    surf    = c.surface2, bar_sym = s.box_bar,
                    grn     = c.green,   bold    = c.bold,
                    ok      = s.success, text    = c.text,
                    RST     = c.reset,
                );
            } else {
                erase_line();
            }
            let _ = io::stdout().flush();
        }

        _ => {}
    }
}

// ── Question callback ─────────────────────────────────────────────────────────

pub fn question_cb(q: AnyQuestion, cfg: &mut Config) {
    match q.question() {
        Question::InstallIgnorepkg(mut q) => {
            warn_msg(&format!("{} is in IgnorePkg — install anyway?", q.pkg().name()), cfg);
            q.set_install(prompt_yn(false, cfg));
        }
        Question::Replace(q) => {
            warn_msg(
                &format!("replace {} with {}/{}?",
                    q.oldpkg().name(), q.newdb().name(), q.newpkg().name()),
                cfg,
            );
            q.set_replace(prompt_yn(false, cfg));
        }
        Question::Conflict(mut q) => {
            let c  = q.conflict();
            let co = &cfg.colors;
            println!(
                "\n  {red}{bold}conflict:{RST} {text}{}{RST} and {text}{}{RST} \
                 conflict ({dim}{}{RST})",
                c.package1().name(), c.package2().name(), c.reason(),
                red = co.red, bold = co.bold, text = co.text,
                dim = co.dim, RST  = co.reset,
            );
            println!("  remove {}?", c.package2().name());
            q.set_remove(prompt_yn(false, cfg));
        }
        Question::RemovePkgs(mut q) => {
            let co = &cfg.colors;
            println!(
                "\n  {yellow}{bold}packages cannot be upgraded (unresolvable deps):{RST}",
                yellow = co.yellow, bold = co.bold, RST = co.reset,
            );
            for p in q.packages() {
                println!(
                    "    {surf}•{RST}  {red}{}{RST}",
                    p.name(), surf = co.surface2, red = co.red, RST = co.reset,
                );
            }
            println!("  skip them?");
            q.set_skip(prompt_yn(false, cfg));
        }
        Question::SelectProvider(mut q) => {
            let pkgs: Vec<_> = q.providers().iter().collect();
            let co = &cfg.colors;
            println!(
                "\n  {mauve}multiple providers for {bold}{}{RST}:",
                q.depend(), mauve = co.mauve, bold = co.bold, RST = co.reset,
            );
            for (i, p) in pkgs.iter().enumerate() {
                println!(
                    "    {surf}[{i}]{RST}  {text}{}{RST}  {dim}{}{RST}",
                    p.name(), p.version(),
                    surf = co.surface2, text = co.text,
                    dim  = co.dim,      RST  = co.reset,
                );
            }
            print!(
                "  {sub}select (0–{}): {RST}",
                pkgs.len().saturating_sub(1),
                sub = co.subtext1, RST = co.reset,
            );
            let _ = io::stdout().flush();
            let idx = read_line().trim().parse::<usize>().unwrap_or(0)
                .min(pkgs.len().saturating_sub(1));
            q.set_index(idx as i32);
        }
        Question::ImportKey(mut q) => {
            let co = &cfg.colors;
            println!(
                "\n  {yellow}import PGP key {bold}{}{RST}?",
                q.fingerprint(), yellow = co.yellow, bold = co.bold, RST = co.reset,
            );
            println!("  {dim}uid: {}{RST}", q.uid(), dim = co.dim, RST = co.reset);
            q.set_import(prompt_yn(true, cfg));
        }
        Question::Corrupted(mut q) => {
            warn_msg(&format!("{} appears corrupted — remove it?", q.filepath()), cfg);
            q.set_remove(prompt_yn(true, cfg));
        }
    }
}

// ── Rendering helpers (pub so main.rs can use them) ───────────────────────────

pub fn render_bar(ratio: f64, col: &str, cfg: &Config) -> String {
    let b = &cfg.bar;
    let c = &cfg.colors;
    let n = (ratio.clamp(0.0, 1.0) * b.width as f64).round() as usize;
    format!(
        "{col}{fill}{RST}{surf}{empty}{RST}",
        fill  = b.fill.repeat(n),
        empty = b.empty.repeat(b.width - n),
        surf  = c.surface1,
        RST   = c.reset,
    )
}

pub fn human_size(bytes: i64) -> String {
    let b = bytes as f64;
    if b < 1024.0 {
        format!("{b:.0} B")
    } else if b < 1024.0 * 1024.0 {
        format!("{:.1} KiB", b / 1024.0)
    } else if b < 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} MiB", b / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GiB", b / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn info_msg(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    println!("  {}{msg}{RST}", c.subtext1, RST = c.reset);
}

pub fn warn_msg(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    println!(
        "  {peach}{bold}{warn}{RST}  {peach}{msg}{RST}",
        peach = c.peach, bold = c.bold, warn = s.warn, RST = c.reset,
    );
}

pub fn error_msg(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    eprintln!(
        "  {red}{bold}{err}{RST}  {red}{msg}{RST}",
        red = c.red, bold = c.bold, err = s.error, RST = c.reset,
    );
}

pub fn success_msg(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    println!(
        "  {grn}{bold}{ok}{RST}  {grn}{msg}{RST}",
        grn = c.green, bold = c.bold, ok = s.success, RST = c.reset,
    );
}

pub fn header_msg(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    println!(
        "\n{mauve}{bold}  {hdr}{RST} {text}{bold}{msg}{RST}",
        mauve = c.mauve, bold = c.bold, hdr  = s.header,
        text  = c.text,  RST  = c.reset,
    );
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn strip_pkg_suffix(s: &str) -> &str {
    s.trim_end_matches(".sig")
     .trim_end_matches(".pkg.tar.zst")
     .trim_end_matches(".pkg.tar.xz")
     .trim_end_matches(".pkg.tar.gz")
     .trim_end_matches(".db.tar.gz")
     .trim_end_matches(".db")
}

fn trunc(s: &str, max: usize) -> &str {
    // avoid splitting on a multi-byte char boundary
    if s.len() <= max { return s; }
    let mut end = max;
    while !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

fn erase_line() {
    print!("\r\x1b[2K");
    let _ = io::stdout().flush();
}

fn status(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    print!(
        "\r\x1b[2K  {surf}{tick}{RST}  {mauve}{msg}…{RST}",
        surf  = c.surface2, tick  = s.box_tick,
        mauve = c.mauve,    RST   = c.reset,
    );
    let _ = io::stdout().flush();
}

fn box_header(msg: &str, cfg: &Config) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    print!("\r\x1b[2K");
    println!(
        "  {surf}{top}{RST} {sub}{bold}{msg}{RST}",
        surf = c.surface2, top  = s.box_top,
        sub  = c.subtext1, bold = c.bold,
        RST  = c.reset,
    );
    let _ = io::stdout().flush();
}

fn prompt_yn(default_yes: bool, cfg: &Config) -> bool {
    if cfg.behavior.noconfirm { return default_yes; }
    let c    = &cfg.colors;
    let hint = if default_yes { "[Y/n]" } else { "[y/N]" };
    print!("  {}{hint}{RST} ", c.subtext1, RST = c.reset);
    let _ = io::stdout().flush();
    let line = read_line();
    let t    = line.trim().to_lowercase();
    if t.is_empty() { default_yes } else { t == "y" || t == "yes" }
}

fn read_line() -> String {
    let mut s = String::new();
    let _ = io::stdin().read_line(&mut s);
    s
}
