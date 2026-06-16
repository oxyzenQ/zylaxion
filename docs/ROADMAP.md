# Zylaxion Roadmap

## Current Status: v0.1.0-pre (pre-release)

Phases 1–6 complete. Repository is structured, CI/CD wired, installer
FHS-compliant, and all pure-Rust tests pass (23/23). Ready for tagging.

---

## Phase 7 — Rule Compliance & Hardening (current)

Target: v0.1.0 release tag.

### 7.1 main.rs modularization
`crates/zylaxion/src/main.rs` is 474 LOC (target: 100–300). Extract into
separate modules:

- `cli.rs` — Clap `Cli`/`Commands` definitions
- `commands/mod.rs` — `cmd_start`, `cmd_daemon`, `cmd_stop`
- `commands/info.rs` — `cmd_doctor`, `cmd_list_profiles`, `cmd_list_backends`
- `profile.rs` — `resolve_profile()` + search path logic

### 7.2 Version output hardening
Fix `--version` / `-V` interception. Current `std::env::args().nth(1)` only
catches positional `version`, not the clap `--version` flag. Use clap's
`long_version` or override `Cli::version()`.

### 7.3 `--check-updated` subcommand
Implement a GitHub Releases API check (`/repos/oxyzenQ/zylaxion/releases/latest`)
using `ureq` or `reqwest`. Compare semver against current version. Print
update available / already up to date.

### 7.4 CI linting
Add `actionlint` and `yamllint` steps to `ci.yml`.

### 7.5 Core engine LOC audit
`zactrix-engine` (694 LOC) + `zactrix-profiles` (903 LOC) = 1,597 LOC total.
Target: <1,000 LOC. Options: inline voice.rs into pool.rs, or extract tests
from production LOC count.

---

## Phase 8 — Audio Enhancements

### 8.1 Keycap type profiles
Per-key profile mapping: space bar gets a deeper thump, modifier keys get
a softer click. Scancode-to-profile table loaded from TOML.

### 8.2 Volume control via IPC
Add `volume <0.0–1.0>` and `mute` commands to the daemon IPC socket.
Wire into `VoicePool::master_volume`.

### 8.3 Hot-swap profile at runtime
Add `profile <name>` IPC command. When received, the daemon reloads the
`MechanicalClick` profile without restarting. Requires `Arc<RwLock<KeyProfile>>`
or atomic swap pattern.

### 8.4 Custom user profiles
Document and validate user profile TOML schema. Add a `zylaxion validate-profile
<path>` subcommand that parses and range-checks DSP parameters.

---

## Phase 9 — Distribution

### 9.1 Arch Linux AUR package
Create `PKGBUILD` for AUR. Contact email: `with dot rezky at gmail dot com`.
Install to `/usr/local` matching FHS.

### 9.2 `.deb` packaging
Create `debian/` control files for Debian/Ubuntu. `dpkg-buildpackage`.

### 9.3 Fedora COPR
Create `zylaxion.spec` for Fedora COPR repository.

---

## Phase 10 — Future Ideas (post-1.0)

- PipeWire-native audio (bypass cpal/ALSA, use pw-loop directly)
- Real-time profile switching via D-Bus
- GUI profile editor (GTK4 or Iced)
- WASAPI backend for cross-platform support
- Per-application volume ducking
