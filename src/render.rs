// Catppuccin Mocha palette + rendering primitives

pub const RST:       &str = "\x1b[0m";
pub const BOLD:      &str = "\x1b[1m";
pub const DIM:       &str = "\x1b[2m";
pub const ITALIC:    &str = "\x1b[3m";

pub const GREEN:     &str = "\x1b[38;2;166;227;161m";
pub const BLUE:      &str = "\x1b[38;2;137;180;250m";
pub const RED:       &str = "\x1b[38;2;243;139;168m";
pub const YELLOW:    &str = "\x1b[38;2;249;226;175m";
pub const MAUVE:     &str = "\x1b[38;2;203;166;247m";
pub const PEACH:     &str = "\x1b[38;2;250;179;135m";
pub const TEAL:      &str = "\x1b[38;2;148;226;213m";
pub const TEXT:      &str = "\x1b[38;2;205;214;244m";
pub const SUBTEXT1:  &str = "\x1b[38;2;186;194;222m";
pub const SUBTEXT0:  &str = "\x1b[38;2;166;173;200m";
pub const SURFACE2:  &str = "\x1b[38;2;88;91;112m";
pub const SURFACE1:  &str = "\x1b[38;2;69;71;90m";
pub const ROSEWATER: &str = "\x1b[38;2;245;224;220m";
pub const FLAMINGO:  &str = "\x1b[38;2;242;205;205m";

pub const BAR_W: usize = 36;

pub fn bar(ratio: f64, col: &str) -> String {
    let n = (ratio.clamp(0.0, 1.0) * BAR_W as f64).round() as usize;
    format!(
        "{}{}{}{}{}",
        col,
        "█".repeat(n),
        SURFACE1,
        "░".repeat(BAR_W - n),
        RST,
    )
}

pub fn header(msg: &str) {
    println!("\n{MAUVE}{BOLD}  ::{RST} {TEXT}{BOLD}{msg}{RST}");
}

pub fn info(msg: &str) {
    println!("  {SUBTEXT1}{msg}{RST}");
}

pub fn warn(msg: &str) {
    println!("  {PEACH}{BOLD}⚠{RST}  {PEACH}{msg}{RST}");
}

pub fn error(msg: &str) {
    eprintln!("  {RED}{BOLD}✗{RST}  {RED}{msg}{RST}");
}

pub fn success(msg: &str) {
    println!("  {GREEN}{BOLD}✓{RST}  {GREEN}{msg}{RST}");
}

pub fn erase_line() {
    print!("\r\x1b[2K");
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
