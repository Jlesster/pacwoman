# pacwoman

> a glamorous, Catppuccin-themed frontend for libalpm

![Rust](https://img.shields.io/badge/rust-2021-b4befe?style=flat-square&logo=rust&logoColor=cdd6f4&labelColor=313244)
![libalpm](https://img.shields.io/badge/libalpm-5-cba6f7?style=flat-square&labelColor=313244)
![license](https://img.shields.io/badge/license-MIT-a6e3a1?style=flat-square&labelColor=313244)
![theme](https://img.shields.io/badge/theme-catppuccin%20mocha-f5c2e7?style=flat-square&labelColor=313244)

pacwoman replaces pacman's sparse output with colour-graded progress bars,
structured install summaries, and a fully configurable symbol + colour palette.
It links directly against `libalpm` — no pacman binary is invoked at runtime.

```
  :: starting full system upgrade
  packages (3):
    ⟳ mesa                          24.3.4-1 → 25.0.1-1   63.21 MiB
    ⟳ lib32-mesa                    24.3.4-1 → 25.0.1-1   32.08 MiB
    ⟳ vulkan-radeon                 24.3.4-1 → 25.0.1-1   12.55 MiB

  total download size:    47.32 MiB
  total installed size:  107.84 MiB
  :: proceed with installation? [Y/n] y

  ┌─ downloading 3 packages
  │  ✓  mesa-25.0.1-1
  │  ✓  lib32-mesa-25.0.1-1
  │  ✓  vulkan-radeon-25.0.1-1

  ( 1/ 3)  ████████████████████████  done  mesa
  ( 2/ 3)  ████████████████████████  done  lib32-mesa
  ( 3/ 3)  ████████████████████████  done  vulkan-radeon

  ┌─ running post-transaction hooks
  ✓  transaction complete
```

---

## Installation

You can install manually or via the install script.

```sh
bash <(curl -fsSL https://raw.githubusercontent.com/Jlesster/pacwoman/main/install.sh)
```

```sh
git clone https://github.com/jless/pacwoman
cd pacwoman
cargo build --release
sudo install -Dm755 target/release/pacwoman /usr/local/bin/pacwoman
```

pacwoman must run as root (or via sudo) for write operations, same as pacman.

Generate the default config:

```sh
sudo pacwoman --gen-config
# → ~/.config/pacwoman/config.json
```

---

## Usage

pacwoman mirrors pacman's flag surface.

| operation      | flags                                        | description                       |
| -------------- | -------------------------------------------- | --------------------------------- |
| `-S pkg …`     |                                              | install or upgrade packages       |
| `-Sy`          |                                              | sync package databases            |
| `-Su`          |                                              | upgrade all installed packages    |
| `-Syu`         |                                              | sync + full upgrade               |
| `-R pkg …`     |                                              | remove packages                   |
| `-Rs pkg …`    |                                              | remove with unneeded dependencies |
| `-Q`           | `-i` `-d` `-e` `-u` `-s term` `-o file` `-q` | query local database              |
| `--gen-config` |                                              | write default config and exit     |

---

## Configuration

Config is read from `$XDG_CONFIG_HOME/pacwoman/config.json`
(default: `~/.config/pacwoman/config.json`).
All sections and keys are optional — missing values fall back to built-in defaults.

### `behavior`

| key                | default | description                                          |
| ------------------ | ------- | ---------------------------------------------------- |
| `noconfirm`        | `false` | skip all Y/n prompts                                 |
| `dl_name_width`    | `24`    | max chars for package name in download bar           |
| `pkg_name_width`   | `28`    | max chars for package name in progress bar           |
| `show_counter`     | `true`  | show `(cur/tot)` badge when installing multiple pkgs |
| `show_summary`     | `true`  | show size summary before confirmation                |
| `show_db_uptodate` | `true`  | print "all databases up to date" when nothing synced |

### `bar`

| key               | default    | description                              |
| ----------------- | ---------- | ---------------------------------------- |
| `width`           | `24`       | total character width of progress bars   |
| `fill`            | `"█"`      | character for the filled portion         |
| `empty`           | `"░"`      | character for the empty portion          |
| `dl_color`        | `"teal"`   | colour role for download bars            |
| `install_color`   | `"blue"`   | colour role for install bars             |
| `remove_color`    | `"red"`    | colour role for remove bars              |
| `upgrade_color`   | `"blue"`   | colour role for upgrade bars             |
| `downgrade_color` | `"yellow"` | colour role for downgrade/reinstall bars |

### `suppress`

| key               | default | description                                    |
| ----------------- | ------- | ---------------------------------------------- |
| `mirror_errors`   | `true`  | hide per-mirror 404/error lines during db sync |
| `mirror_warnings` | `true`  | hide "too many errors from X" warnings         |
| `hook_names`      | `false` | hide individual hook names (show header only)  |
| `scriptlet`       | `false` | hide install scriptlet output                  |
| `optdep_removal`  | `false` | hide optional dependency removal notices       |
| `pacnew`          | `false` | hide pacnew/pacsave creation notices           |

### `symbols`

| key         | default  | description                        |
| ----------- | -------- | ---------------------------------- |
| `install`   | `"↑"`    | prefix for install operations      |
| `upgrade`   | `"⟳"`    | prefix for upgrade operations      |
| `downgrade` | `"↓"`    | prefix for downgrade operations    |
| `reinstall` | `"↺"`    | prefix for reinstall operations    |
| `remove`    | `"✕"`    | prefix for remove operations       |
| `success`   | `"✓"`    | success status indicator           |
| `error`     | `"✗"`    | error status indicator             |
| `warn`      | `"⚠"`    | warning status indicator           |
| `download`  | `"↓"`    | download indicator                 |
| `done`      | `"done"` | completion label on progress bars  |
| `box_top`   | `"┌─"`   | section header box-drawing prefix  |
| `box_bar`   | `"│"`    | download row box-drawing prefix    |
| `box_tick`  | `"┄"`    | hook/status row box-drawing prefix |
| `header`    | `"::"`   | section header prefix              |
| `bullet`    | `"•"`    | package list bullet                |

### `colors`

Each field accepts a hex colour in `"rrggbb"` or `"rrggbbaa"` format.
Defaults are [Catppuccin Mocha](https://github.com/catppuccin/catppuccin).

| key         | default    | Mocha name |
| ----------- | ---------- | ---------- |
| `green`     | `"a6e3a1"` | Green      |
| `blue`      | `"89b4fa"` | Blue       |
| `red`       | `"f38ba8"` | Red        |
| `yellow`    | `"f9e2af"` | Yellow     |
| `mauve`     | `"cba6f7"` | Mauve      |
| `peach`     | `"fab387"` | Peach      |
| `teal`      | `"94e2d5"` | Teal       |
| `text`      | `"cdd6f4"` | Text       |
| `subtext1`  | `"bac2de"` | Subtext 1  |
| `subtext0`  | `"a6adc8"` | Subtext 0  |
| `surface2`  | `"585b70"` | Surface 2  |
| `surface1`  | `"45475a"` | Surface 1  |
| `rosewater` | `"f5e0dc"` | Rosewater  |
| `flamingo`  | `"f2cdcd"` | Flamingo   |

#### Switching flavours

Drop in any Catppuccin flavour (or your own palette) by swapping the hex values.
The full Catppuccin port palette is at
[catppuccin/catppuccin](https://github.com/catppuccin/catppuccin).

#### Color roles

Bar colour fields accept one of the named roles defined in `colors`:

`green` `blue` `red` `yellow` `mauve` `peach` `teal`
`text` `subtext1` `subtext0` `surface2` `surface1` `rosewater` `flamingo`

---

## Dependencies

| crate        | reason                       |
| ------------ | ---------------------------- |
| `alpm`       | libalpm bindings             |
| `libc`       | memfd / low-level primitives |
| `serde`      | config derive                |
| `serde_json` | config parsing               |

No async runtime, no TUI framework, no HTTP client.

---

## License

MIT — see [`LICENSE`](LICENSE).

---

<sub>themed with <a href="https://github.com/catppuccin/catppuccin">Catppuccin Mocha 🐱</a></sub>
