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

// ── Top-level config ──────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub colors:   ColorConfig,
    pub bar:      BarConfig,
    pub symbols:  SymbolConfig,
    pub suppress: SuppressConfig,
    pub behavior: BehaviorConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            colors:   ColorConfig::default(),
            bar:      BarConfig::default(),
            symbols:  SymbolConfig::default(),
            suppress: SuppressConfig::default(),
            behavior: BehaviorConfig::default(),
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        if !path.exists() {
            return Self::default();
        }
        let raw = match fs::read_to_string(&path) {
            Ok(s)  => s,
            Err(e) => {
                eprintln!("pacwoman: could not read config {}: {e}", path.display());
                return Self::default();
            }
        };
        match serde_json::from_str(&raw) {
            Ok(cfg) => cfg,
            Err(e)  => {
                eprintln!("pacwoman: config parse error: {e}");
                Self::default()
            }
        }
    }

    /// Write the default config to XDG path (for --gen-config)
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

// ── Colors ────────────────────────────────────────────────────────────────────

/// Each field is an ANSI escape sequence string.
/// Defaults are Catppuccin Mocha.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorConfig {
    pub reset:     String,
    pub bold:      String,
    pub dim:       String,

    // Catppuccin Mocha palette
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

impl Default for ColorConfig {
    fn default() -> Self {
        Self {
            reset:     "\x1b[0m".into(),
            bold:      "\x1b[1m".into(),
            dim:       "\x1b[2m".into(),
            green:     "\x1b[38;2;166;227;161m".into(),
            blue:      "\x1b[38;2;137;180;250m".into(),
            red:       "\x1b[38;2;243;139;168m".into(),
            yellow:    "\x1b[38;2;249;226;175m".into(),
            mauve:     "\x1b[38;2;203;166;247m".into(),
            peach:     "\x1b[38;2;250;179;135m".into(),
            teal:      "\x1b[38;2;148;226;213m".into(),
            text:      "\x1b[38;2;205;214;244m".into(),
            subtext1:  "\x1b[38;2;186;194;222m".into(),
            subtext0:  "\x1b[38;2;166;173;200m".into(),
            surface2:  "\x1b[38;2;88;91;112m".into(),
            surface1:  "\x1b[38;2;69;71;90m".into(),
            rosewater: "\x1b[38;2;245;224;220m".into(),
            flamingo:  "\x1b[38;2;242;205;205m".into(),
        }
    }
}

// ── Bar ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BarConfig {
    /// Total character width of the progress bar
    pub width:    usize,
    /// Character used for the filled portion
    pub fill:     String,
    /// Character used for the empty portion
    pub empty:    String,
    /// Color role for download bars (key into ColorConfig)
    pub dl_color: ColorRole,
    /// Color role for install bars
    pub install_color:   ColorRole,
    /// Color role for remove bars
    pub remove_color:    ColorRole,
    /// Color role for upgrade bars
    pub upgrade_color:   ColorRole,
    /// Color role for downgrade/reinstall bars
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

/// Named color roles that map to fields in ColorConfig.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorRole {
    Green, Blue, Red, Yellow, Mauve, Peach, Teal,
    Text, Subtext1, Subtext0, Surface2, Surface1,
    Rosewater, Flamingo,
}

impl ColorRole {
    pub fn resolve<'a>(&self, c: &'a ColorConfig) -> &'a str {
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
    // operation prefixes
    pub install:   String,
    pub upgrade:   String,
    pub downgrade: String,
    pub reinstall: String,
    pub remove:    String,

    // status indicators
    pub success:   String,
    pub error:     String,
    pub warn:      String,
    pub download:  String,
    pub done:      String,

    // box drawing
    pub box_top:   String,  // ┌─
    pub box_bar:   String,  // │
    pub box_tick:  String,  // ┄

    // section header prefix  (:: msg)
    pub header:    String,

    // info/bullet
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
    /// Hide per-mirror 404/error lines during db sync
    pub mirror_errors:   bool,
    /// Hide "too many errors from X" warnings
    pub mirror_warnings: bool,
    /// Hide hook run names (just show the section header)
    pub hook_names:      bool,
    /// Hide scriptlet output
    pub scriptlet:       bool,
    /// Hide optdep removal notices
    pub optdep_removal:  bool,
    /// Hide pacnew/pacsave notices
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
    /// Always answer yes to prompts (equivalent to --noconfirm)
    pub noconfirm:          bool,
    /// Max chars to show for package name in download bar
    pub dl_name_width:      usize,
    /// Max chars to show for package name in progress bar
    pub pkg_name_width:     usize,
    /// Show (cur/tot) counter badge when installing multiple packages
    pub show_counter:       bool,
    /// Show total download / install size summary before confirmation
    pub show_summary:       bool,
    /// Show "all databases are up to date" message when nothing changed
    pub show_db_uptodate:   bool,
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
