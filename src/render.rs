use crate::config::ResolvedConfig;

// Catppuccin Mocha palette + rendering primitives
// Kept as hardcoded fallbacks for direct use in query.rs format strings.

pub const RST:       &str = "\x1b[0m";
pub const BOLD:      &str = "\x1b[1m";
pub const DIM:       &str = "\x1b[2m";

pub const GREEN:     &str = "\x1b[38;2;166;227;161m";
pub const BLUE:      &str = "\x1b[38;2;137;180;250m";
pub const RED:       &str = "\x1b[38;2;243;139;168m";
pub const YELLOW:    &str = "\x1b[38;2;249;226;175m";
pub const MAUVE:     &str = "\x1b[38;2;203;166;247m";
pub const TEXT:      &str = "\x1b[38;2;205;214;244m";
pub const SUBTEXT1:  &str = "\x1b[38;2;186;194;222m";
pub const SURFACE2:  &str = "\x1b[38;2;88;91;112m";
pub const ROSEWATER: &str = "\x1b[38;2;245;224;220m";

pub fn header(msg: &str, cfg: &ResolvedConfig) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    if cfg.plain {
        println!("\n:: {}", msg);
    } else {
        println!(
            "\n{mauve}{bold}  {hdr}{reset} {text}{bold}{msg}{reset}",
            mauve = c.mauve, bold = c.bold, hdr  = s.header,
            text  = c.text,  reset = c.reset,
        );
    }
}

pub fn info(msg: &str, cfg: &ResolvedConfig) {
    if cfg.plain {
        println!("{}", msg);
    } else {
        println!("  {sub}{msg}{reset}", sub = cfg.colors.subtext1, reset = cfg.colors.reset);
    }
}

pub fn warn(msg: &str, cfg: &ResolvedConfig) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    if cfg.plain {
        println!("WARNING: {}", msg);
    } else {
        println!(
            "  {peach}{bold}{sym}{reset}  {peach}{msg}{reset}",
            peach = c.peach, bold = c.bold, sym = s.warn, reset = c.reset,
        );
    }
}

pub fn error(msg: &str, cfg: &ResolvedConfig) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    if cfg.plain {
        eprintln!("  ERROR: {}", msg);
    } else {
        eprintln!(
            "  {red}{bold}{sym}{reset}  {red}{msg}{reset}",
            red = c.red, bold = c.bold, sym = s.error, reset = c.reset,
        );
    }
}

pub fn success(msg: &str, cfg: &ResolvedConfig) {
    let c = &cfg.colors;
    let s = &cfg.symbols;
    if cfg.plain {
        println!("  [OK] {}", msg);
    } else {
        println!(
            "  {green}{bold}{sym}{reset}  {green}{msg}{reset}",
            green = c.green, bold = c.bold, sym = s.success, reset = c.reset,
        );
    }
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

pub fn kv(key: &str, val: &str, cfg: &ResolvedConfig) {
    let c = &cfg.colors;
    if cfg.plain {
        println!("{key:<18}: {val}");
    } else {
        println!(
            "  {mauve}{bold}{key:<18}{reset} {text}{val}{reset}",
            mauve = c.mauve, bold = c.bold, text = c.text, reset = c.reset,
        );
    }
}
