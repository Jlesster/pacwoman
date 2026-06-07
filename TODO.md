# pacwoman → pacman drop-in readiness

Priority legend:
- **BLOCKER** — AUR helpers (`yay`/`paru`/`pikaur`) **will fail** without this
- **HIGH** — commonly used by scripts/tooling, needed for real-world swap
- **MEDIUM** — AUR helpers pass these through to subprocess pacman
- **LOW** — edge-case flags, nice-to-have

---

## 🚫 BLOCKER — breaks AUR helpers

### 1. `-Qu` flag collision
`src/main.rs:117` — `-Qk` currently maps to upgrades query. Real pacman:
- `-Qu` — query upgradable packages
- `-Qk` — check package file integrity

`yay`, `paru`, `pikaur` all call `pacman -Qu` / `pacman -Quq` to list updates.
Reassign `u` → upgrades in query context; actually implement `-Qk` for file-checking.

### 2. `--print-format` not implemented
All three helpers use `pacman -Sp --print-format "%r/%n"` during dependency
resolution to determine which repository a package belongs to. pacwoman errors
on any unknown flag (`src/main.rs:95`) → hard abort mid-resolution.

Implies `-p` / `--print` as well. Needs format specifiers `%r`, `%n`, `%v`, `%l`,
`%s`, `%a`, etc.

### 3. `--needed` not implemented
Passed by helpers during `-S` to skip reinstalling up-to-date packages.
Without it, behavior silently diverges from pacman.

### 4. `--overwrite` not implemented
Passed by helpers for file conflict resolution. Some user workflows pass it
through as well.

---

## 🔴 HIGH — scripts & common tooling break

### 5. `-T` / `--deptest` not implemented
`makepkg -s` uses this to check installed dependencies. `pikaur` calls
`pacman --deptest` to find missing locally-installed deps. Returns a
newline-separated list of unsatisfied dependencies on stdout.

### 6. `-p` / `--print` not implemented
Dry-run: print what would happen without actually doing it. Implied by
`--print-format`.

### 7. `--noprogressbar` not implemented
Scripts need this to avoid ANSI progress-bar escape sequences in captured
output.

### 8. `-V` / `--version` not implemented
Should print `pacwoman v0.1.4` (or similar version string) and exit **2**
(pacman exit code convention for informative operations).

### 9. `-h` / `--help` not implemented
Should print operation-specific help text and exit **2**.

### 10. `-S` sub-flags missing
- `-Si` / `--info` — sync package info (helpers use this for remote pkg metadata)
- `-Sl` / `--list` — list all packages in specified repos
- `-Ss` / `--search` — search sync databases (foundational for finding packages)
- `-Sc` / `-Scc` — clean package cache (single = unused only, double = all)
- `-Sg` / `--groups` — list package groups and their members
- `-Sq` / `--quiet` — less verbose output

### 11. `-Q` sub-flags missing or wrong
- `-Qk` — needs re-assignment: currently maps to upgrades (wrong), should check
  file integrity (check all files owned by package are present)
- `-Ql` / `--list` — list files owned by package
- `-Qm` / `--foreign` — packages not found in any sync DB (**critical**: AUR
  helpers use this to identify AUR-installed packages)
- `-Qn` / `--native` — inverse of foreign (packages found in sync DBs)
- `-Qg` / `--groups` — list group members
- `-Qc` / `--changelog` — view package changelog
- `-Qp` / `--file` — query an actual package file, not a DB entry
- `-x` / `--regex` — for `-F` files operation

### 12. `-R` sub-flags missing
- `-Rc` / `--cascade` — remove cascade (also removes packages that depend on
  the target)
- `-Ru` / `--unneeded` — only remove targets that are not required by other
  packages

### 13. `-D` sub-flags missing
- `-Dk` / `--check` — check local DB for consistency
- `-Dq` / `--quiet` — suppress success messages

### 14. `-F` / `--files` operation entirely missing
Queries `.files` databases (separate from `-Qo` which only checks the local DB).
- `-Fy` — refresh file databases
- `-Fl` — list files owned by queried package
- `-Fx` — interpret query as regex
- `-Fq` — quiet
- `--machinereadable` — NULL-delimited output for scripts

### 15. Exit code 2 for `--help` / `--version`
pacman exits **2** (not 0 or 1) for `--help` and `--version`. pacwoman currently
uses `process::exit(1)` for all errors and 0 for success. Scripts and some
tooling check for exit code 2 to distinguish "informational" from "error".

---

## 🟡 MEDIUM — AUR helpers pass these through

### 16. `--ignore` / `--ignoregroup` (CLI overrides)
Currently read from pacman.conf only (`src/main.rs:230-234`). CLI overrides
aren't accepted, but helpers may pass them.

### 17. `--nodeps` / `-d`
Skip dependency version checks. `-dd` skips all dep checks. Used for recovery.

### 18. `--assume-installed` `<package=version>`
Add a virtual package to satisfy dependencies without disabling all dep checks.

### 19. `--dbonly`
Add/remove database entry only, leave all files in place.

### 20. `--noscriptlet`
Skip execution of install scriptlets.

### 21. `--config` `<path>`
Helpers (especially paru via `src/exec.rs`) pass `--config /etc/pacman.conf`
explicitly. Hardcoded to `/etc/pacman.conf` currently (`src/main.rs:224`).

### 22. `--dbpath`, `--root`, `--cachedir`, `--gpgdir`, `--hookdir`, `--logfile`
All hardcoded (`src/main.rs:17-21`). pikaur passes these through to subprocess
pacman. Helpers that call subprocess pacman may pass any of these.

### 23. `--arch` `<arch>`
Alternate architecture for package resolution.

### 24. `--color` `<always|never|auto>`
pikaur forces `--color=always` or `--color=never`. pacwoman should respect this
and set `cli.plain` accordingly.

### 25. `--confirm`
Cancels a prior `--noconfirm` in the argument list.

### 26. `-v` / `--verbose`
Print paths (root, conf file, DB path, cache dirs, etc.) and exit.

---

## 🟢 LOW — advanced / rare / nice-to-have

### 27. `--sysroot` `<dir>` for guest system operations

### 28. `--disable-sandbox`, `--disable-sandbox-filesystem`, `--disable-sandbox-syscalls`
New in pacman 7.0 — download process sandboxing.

### 29. `--disable-download-timeout`
Disable low-speed limit and timeout on downloads.

### 30. `-uu` — enable downgrades during sysupgrade
Pass `--sysupgrade` twice. Currently only `cli.refresh` has a counter;
`sysupgrade` is bool.

### 31. `-dd` — double nodeps (skip all dependency checks)

### 32. `-Rss` — recursive removal including explicitly installed packages

### 33. `-Syy` — force database refresh (already partially works via `refresh: u8`)

---

## 📐 Output compatibility

### 34. `-Qi` plain-text format mismatch
Scripts parse `pacman -Qi` with grep/awk for fields like `Version :`,
`Description :`, `Depends On :`, etc. pacwoman's ANSI-colored output and
different KV layout (`src/query.rs:139-162`) breaks text parsing. The
`cli.plain` flag is set when stdout isn't a TTY, but `render.rs` constants
are still used in some `-Qi` paths rather than plain text.

### 35. `-Qo` output format
Currently: `file is owned by pkg version` (with ANSI foreground codes).
Real pacman: `file is owned by pkgname pkgver` (plain).

### 36. `-Qs` / `-Ss` format
Differs from pacman's `repo/pkgname pkgver (+ groups) description` output.

### 37. `--quiet` consistency
- `-Qq` — just package names (already works)
- `-Sq` / `-Fq` — need implementation

### 38. Machine-readable output parity
- `--print-format` for structured output (BLOCKER)
- `--machinereadable` for `-F` operation
- No JSON output in pacman, but custom format strings cover helper needs

---

## ⚙️ pacman.conf options not parsed

### 39. Additional `[options]` not respected
Currently reads: IgnorePkg, IgnoreGroup, NoUpgrade, NoExtract
(`src/main.rs:230-241`).

Missing options that affect behavior:
- `ParallelDownloads` — parallel download count
- `CacheDir` — alternative cache location
- `Architecture` — auto or multi-lib
- `Color` — `Always`/`Never`/`Auto`
- `SigLevel` — `PackageOptional`/`PackageRequired`/`DatabaseOptional`/
  `DatabaseRequired`/`TrustedOnly`/`TrustAll`
- `LocalFileSigLevel` — signature checking for `-U`
- `RemoteFileSigLevel` — signature checking for `-S`
- `HoldPkg` — packages protected from `-R`
- `DisableDownloadTimeout` — disable low speed limit
- `DownloadUser` — drop privileges for downloads
- `VerbosePkgLists` — detailed package lists
- `ILoveCandy` — animated progress bar
- `TotalDownload` — per-repo download totals
- `CheckSpace` — check available disk space before installing

---

## 🔒 Symlink safety at `/usr/local/bin/pacman`

| Aspect | Status | Notes |
|--------|--------|-------|
| libalpm compatibility | ✅ Safe | Same C library, same transactions |
| Lock file | ✅ Safe | Same `/var/lib/pacman/db.lck` |
| Self-update resolves symlinks | ✅ Safe | `canonicalize()` at `src/main.rs:1102-1103` |
| `-Syu` muscle memory | ✅ Works | Full sysupgrade via libalpm |
| `-S pkg` | ✅ Works | Sync install via libalpm |
| `-R pkg` | ✅ Works | Remove via libalpm |
| `-U file.pkg.tar.zst` | ✅ Works | Local install via libalpm |
| `-Qu` from muscle memory | ❌ Broken | `-Qu` does nothing currently |
| AUR helper `yay -Syu` | ❌ Breaks | Dies on `--print-format` + `-Qu` |
| `checkupdates` script | ❌ Breaks | Calls `pacman -Qu` |
| `makepkg -s` dependency check | ❌ Breaks | Uses `--deptest` |
| `pacdiff` / `paccache` | ✅ Works | Don't invoke pacman binary |
| `pactree` | ✅ Works | Uses libalpm directly |

---

## Recommended implementation order

```
Phase 1 (blockers — AUR helpers work)
  ├── Fix -Qu flag (reassign from -Qk, actually implement -Qk file check)
  ├── Implement -V / --version + exit code 2
  ├── Implement -h / --help per-operation + exit code 2
  └── Implement --print-format + -Sp / --print

Phase 2 (scripts & common commands work)
  ├── Implement -T / --deptest
  ├── Implement -Ss / -Sl / -Si / -Sc / -Sg / -Sq (sync sub-flags)
  ├── Implement -Qm / -Qn / -Qk / -Ql / -Qg / -Qc / -Qp (query sub-flags)
  ├── Implement -Rc / -Ru
  ├── Implement -Dk
  ├── Implement --needed
  └── Implement --noprogressbar

Phase 3 (full passthrough compatibility)
  ├── Implement --config / --dbpath / --root / --cachedir / --gpgdir
  ├── Implement --overwrite
  ├── Implement --nodeps / --assume-installed / --dbonly / --noscriptlet
  ├── Implement --ignore / --ignoregroup (CLI overrides)
  ├── Implement --color
  └── Implement -F operation (files DB)

Phase 4 (polish)
  ├── Verify plain mode is truly ANSI-free in all output paths
  ├── Match -Qi / -Qo / -Qs plain-text output format to pacman
  ├── Parse more pacman.conf options (ParallelDownloads, SigLevel, etc.)
  ├── Implement -uu downgrade support
  └── Implement -ss / -dd double-flag semantics
```
