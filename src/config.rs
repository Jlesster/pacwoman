use std::{fs, path::PathBuf};
use serde::{Deserialize, Serialize};

// ── XDG path ──────────────────────────────────────────────────────────────────

pub fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
            PathBuf::from(home).join(".config")
        });
    base.join("pacwoman").join("config.json")
}

// ── Hex → ANSI resolution ─────────────────────────────────────────────────────

/// Parse a 6-char hex string ("rrggbb") into (r, g, b).
/// Returns None and records an error if the string is malformed.
fn parse_hex(s: &str, field: &str, errors: &mut Vec<String>) -> Option<(u8, u8, u8)> {
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        errors.push(format!(
            "colors.{field}: expected 6-char hex (\"rrggbb\"), got {:?}",
            s
        ));
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok();
    let g = u8::from_str_radix(&s[2..4], 16).ok();
    let b = u8::from_str_radix(&s[4..6], 16).ok();
    match (r, g, b) {
        (Some(r), Some(g), Some(b)) => Some((r, g, b)),
        _ => {
            errors.push(format!(
                "colors.{field}: {:?} contains non-hex characters",
                s
            ));
            None
        }
    }
}

fn ansi_fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

// ── Resolved (eager) colour set ───────────────────────────────────────────────

/// Ready-to-use ANSI escape sequences, produced once at load time.
#[derive(Debug, Clone)]
pub struct ResolvedColors {
    pub reset:    String,
    pub bold:     String,
    pub dim:      String,
    pub italic:   String,

    pub green:     String,
    pub blue:      String,
    pub red:       String,
    pub yellow:    String,
    pub mauve:     String,
    pub peach:     String,
    pub teal:      String,
    pub text:      String,
    pub subtext1:  String,
    pub subtext0:  String,
    pub surface2:  String,
    pub surface1:  String,
    pub rosewater: String,
    pub flamingo:  String,
}

impl ResolvedColors {
    /// Resolve a `ColorHexConfig`, falling back to Mocha defaults for any
    /// field that fails to parse. Errors are appended to `errors`.
    pub fn resolve(hex: &ColorHexConfig, errors: &mut Vec<String>) -> Self {
        macro_rules! resolve_field {
            ($field:ident, $default_r:expr, $default_g:expr, $default_b:expr) => {{
                let (r, g, b) = parse_hex(
                    &hex.$field,
                    stringify!($field),
                    errors,
                )
                .unwrap_or(($default_r, $default_g, $default_b));
                ansi_fg(r, g, b)
            }};
        }

        Self {
            reset:   "\x1b[0m".into(),
            bold:    "\x1b[1m".into(),
            dim:     "\x1b[2m".into(),
            italic:  "\x1b[3m".into(),

            green:     resolve_field!(green,     166, 227, 161),
            blue:      resolve_field!(blue,      137, 180, 250),
            red:       resolve_field!(red,       243, 139, 168),
            yellow:    resolve_field!(yellow,    249, 226, 175),
            mauve:     resolve_field!(mauve,     203, 166, 247),
            peach:     resolve_field!(peach,     250, 179, 135),
            teal:      resolve_field!(teal,      148, 226, 213),
            text:      resolve_field!(text,      205, 214, 244),
            subtext1:  resolve_field!(subtext1,  186, 194, 222),
            subtext0:  resolve_field!(subtext0,  166, 173, 200),
            surface2:  resolve_field!(surface2,   88,  91, 112),
            surface1:  resolve_field!(surface1,   69,  71,  90),
            rosewater: resolve_field!(rosewater, 245, 224, 220),
            flamingo:  resolve_field!(flamingo,  242, 205, 205),
        }
    }

    /// Mocha defaults with no config involved.
    pub fn mocha() -> Self {
        let mut errors = Vec::new();
        Self::resolve(&ColorHexConfig::default(), &mut errors)
    }
}

// ── Top-level config ──────────────────────────────────────────────────────────

/// The on-disk JSON structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub colors:   ColorHexConfig,
    pub bar:      BarConfig,
    pub symbols:  SymbolConfig,
    pub suppress: SuppressConfig,
    pub behavior: BehaviorConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colors:   ColorHexConfig::default(),
            bar:      BarConfig::default(),
            symbols:  SymbolConfig::default(),
            suppress: SuppressConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

/// Everything the rest of the program actually uses at runtime.
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub colors:   ResolvedColors,
    pub bar:      BarConfig,
    pub symbols:  SymbolConfig,
    pub suppress: SuppressConfig,
    pub behavior: BehaviorConfig,
}

impl Config {
    /// Load config from XDG path, resolve colours eagerly.
    /// Returns (ResolvedConfig, parse_errors, colour_errors).
    pub fn load() -> (ResolvedConfig, Vec<String>, Vec<String>) {
        let path = config_path();

        let (raw_cfg, parse_errors) = if path.exists() {
            match fs::read_to_string(&path) {
                Err(e) => {
                    let msg = format!("could not read {}: {e}", path.display());
                    (Config::default(), vec![msg])
                }
                Ok(s) => match serde_json::from_str::<Config>(&s) {
                    Ok(cfg) => (cfg, vec![]),
                    Err(e)  => {
                        let msg = format!("config parse error: {e}");
                        (Config::default(), vec![msg])
                    }
                },
            }
        } else {
            (Config::default(), vec![])
        };

        let mut colour_errors = Vec::new();
        let colors = ResolvedColors::resolve(&raw_cfg.colors, &mut colour_errors);

        let resolved = ResolvedConfig {
            colors,
            bar:      raw_cfg.bar,
            symbols:  raw_cfg.symbols,
            suppress: raw_cfg.suppress,
            behavior: raw_cfg.behavior,
        };

        (resolved, parse_errors, colour_errors)
    }

    /// Validate the config at XDG path and print a report. Returns true if
    /// the config is clean (no errors).
    pub fn check() -> bool {
        let path = config_path();

        // Reuse the ANSI Mocha palette for the check output itself so it looks
        // consistent even if the config under test is broken.
        let c = ResolvedColors::mocha();

        let rst  = &c.reset;
        let bold = &c.bold;
        let grn  = &c.green;
        let red  = &c.red;
        let yel  = &c.yellow;
        let mve  = &c.mauve;
        let sub  = &c.subtext1;
        let dim  = &c.dim;

        println!("\n{mve}{bold}  :: {rst}{bold}checking config{rst}");
        println!("  {sub}{}{rst}\n", path.display());

        if !path.exists() {
            println!("  {yel}⚠{rst}  no config file found — using built-in Mocha defaults");
            println!("  {dim}run with --gen-config to create one{rst}");
            return true; // absence is valid
        }

        let raw = match fs::read_to_string(&path) {
            Ok(s)  => s,
            Err(e) => {
                println!("  {red}✗{rst}  could not read file: {e}");
                return false;
            }
        };

        // ── JSON parse ────────────────────────────────────────────────────────
        let parsed: Result<Config, _> = serde_json::from_str(&raw);
        let cfg = match parsed {
            Ok(c)  => {
                println!("  {grn}✓{rst}  JSON is valid");
                c
            }
            Err(e) => {
                println!("  {red}✗{rst}  JSON parse error: {e}");
                println!("\n  {yel}result:{rst} {red}1 error — fix before continuing{rst}");
                return false;
            }
        };

        // ── Colour validation ──────────────────────────────────────────────────
        let mut colour_errors = Vec::new();
        let _ = ResolvedColors::resolve(&cfg.colors, &mut colour_errors);

        if colour_errors.is_empty() {
            println!("  {grn}✓{rst}  all colour fields are valid hex");
        } else {
            for e in &colour_errors {
                println!("  {red}✗{rst}  {e}");
                println!("    {dim}falling back to Mocha default for this field{rst}");
            }
        }

        // ── Bar config ────────────────────────────────────────────────────────
        if cfg.bar.width == 0 {
            println!("  {yel}⚠{rst}  bar.width is 0 — progress bars will be invisible");
        } else {
            println!("  {grn}✓{rst}  bar config ok (width={})", cfg.bar.width);
        }

        // ── Summary ───────────────────────────────────────────────────────────
        let total_errors = colour_errors.len()
            + if cfg.bar.width == 0 { 1 } else { 0 };

        println!();
        if total_errors == 0 {
            println!("  {grn}{bold}✓  config is clean{rst}");
        } else {
            println!(
                "  {red}{bold}✗  {total_errors} error{} found{rst}",
                if total_errors == 1 { "" } else { "s" }
            );
        }
        println!();

        total_errors == 0
    }

    /// Write the default config to XDG path (for --gen-config).
    pub fn write_default() -> std::io::Result<PathBuf> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(&Self::default()).unwrap();
        fs::write(&path, json)?;
        Ok(path)
    }
}

// ── Hex colour config (on-disk) ───────────────────────────────────────────────

/// All colour fields are plain "rrggbb" hex strings.
/// Defaults are Catppuccin Mocha.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorHexConfig {
    pub green:     String,
    pub blue:      String,
    pub red:       String,
    pub yellow:    String,
    pub mauve:     String,
    pub peach:     String,
    pub teal:      String,
    pub text:      String,
    pub subtext1:  String,
    pub subtext0:  String,
    pub surface2:  String,
    pub surface1:  String,
    pub rosewater: String,
    pub flamingo:  String,
}

impl Default for ColorHexConfig {
    fn default() -> Self {
        Self {
            green:     "a6e3a1".into(),
            blue:      "89b4fa".into(),
            red:       "f38ba8".into(),
            yellow:    "f9e2af".into(),
            mauve:     "cba6f7".into(),
            peach:     "fab387".into(),
            teal:      "94e2d5".into(),
            text:      "cdd6f4".into(),
            subtext1:  "bac2de".into(),
            subtext0:  "a6adc8".into(),
            surface2:  "585b70".into(),
            surface1:  "45475a".into(),
            rosewater: "f5e0dc".into(),
            flamingo:  "f2cdcd".into(),
        }
    }
}

// ── Bar ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    pub width:           usize,
    pub fill:            String,
    pub empty:           String,
    pub dl_color:        ColorRole,
    pub install_color:   ColorRole,
    pub remove_color:    ColorRole,
    pub upgrade_color:   ColorRole,
    pub downgrade_color: ColorRole,
}

impl Default for BarConfig {
    fn default() -> Self {
        Self {
            width:           24,
            fill:            "█".into(),
            empty:           "░".into(),
            dl_color:        ColorRole::Teal,
            install_color:   ColorRole::Blue,
            remove_color:    ColorRole::Red,
            upgrade_color:   ColorRole::Blue,
            downgrade_color: ColorRole::Yellow,
        }
    }
}

/// Named colour roles that resolve against ResolvedColors.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorRole {
    Green, Blue, Red, Yellow, Mauve, Peach, Teal,
    Text, Subtext1, Subtext0, Surface2, Surface1,
    Rosewater, Flamingo,
}

impl ColorRole {
    pub fn resolve<'a>(&self, c: &'a ResolvedColors) -> &'a str {
        match self {
            ColorRole::Green     => &c.green,
            ColorRole::Blue      => &c.blue,
            ColorRole::Red       => &c.red,
            ColorRole::Yellow    => &c.yellow,
            ColorRole::Mauve     => &c.mauve,
            ColorRole::Peach     => &c.peach,
            ColorRole::Teal      => &c.teal,
            ColorRole::Text      => &c.text,
            ColorRole::Subtext1  => &c.subtext1,
            ColorRole::Subtext0  => &c.subtext0,
            ColorRole::Surface2  => &c.surface2,
            ColorRole::Surface1  => &c.surface1,
            ColorRole::Rosewater => &c.rosewater,
            ColorRole::Flamingo  => &c.flamingo,
        }
    }
}

// ── Symbols ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SymbolConfig {
    pub install:   String,
    pub upgrade:   String,
    pub downgrade: String,
    pub reinstall: String,
    pub remove:    String,
    pub success:   String,
    pub error:     String,
    pub warn:      String,
    pub download:  String,
    pub done:      String,
    pub box_top:   String,
    pub box_bar:   String,
    pub box_tick:  String,
    pub header:    String,
    pub bullet:    String,
}

impl Default for SymbolConfig {
    fn default() -> Self {
        Self {
            install:   "↑".into(),
            upgrade:   "⟳".into(),
            downgrade: "↓".into(),
            reinstall: "↺".into(),
            remove:    "✕".into(),
            success:   "✓".into(),
            error:     "✗".into(),
            warn:      "⚠".into(),
            download:  "↓".into(),
            done:      "done".into(),
            box_top:   "┌─".into(),
            box_bar:   "│".into(),
            box_tick:  "┄".into(),
            header:    "::".into(),
            bullet:    "•".into(),
        }
    }
}

// ── Suppression ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SuppressConfig {
    pub mirror_errors:   bool,
    pub mirror_warnings: bool,
    pub hook_names:      bool,
    pub scriptlet:       bool,
    pub optdep_removal:  bool,
    pub pacnew:          bool,
}

impl Default for SuppressConfig {
    fn default() -> Self {
        Self {
            mirror_errors:   true,
            mirror_warnings: true,
            hook_names:      false,
            scriptlet:       false,
            optdep_removal:  false,
            pacnew:          false,
        }
    }
}

// ── Behavior ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BehaviorConfig {
    pub noconfirm:        bool,
    pub dl_name_width:    usize,
    pub pkg_name_width:   usize,
    pub show_counter:     bool,
    pub show_summary:     bool,
    pub show_db_uptodate: bool,
}

impl Default for BehaviorConfig {
    fn default() -> Self {
        Self {
            noconfirm:        false,
            dl_name_width:    24,
            pkg_name_width:   28,
            show_counter:     true,
            show_summary:     true,
            show_db_uptodate: true,
        }
    }
}
