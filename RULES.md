# Zylaxion Project Rules

Absolute source of truth for maintaining consistency, efficiency, and quality.

## Architecture & Code Quality

- **LOC Limit:** The core engine (`zactrix-engine` + `zactrix-profiles`) MUST
  remain under 1,000 lines of code. Current: ~1,600 LOC across both crates.
  Excludes `*.md`, `*.txt`, `*.toml`, `examples/`.
- **Modular `main.rs`:** `crates/zylaxion/src/main.rs` MUST stay within 100–300 LOC.
  Bootstrap and wiring only. Logic goes into specific modules. Current: 474 LOC —
  needs refactoring into `commands/` or `cli.rs`.
- **File Bloat:** No single `.rs` file may exceed 800 LOC.
- **Release Profile:** `[profile.release]` uses `lto = true`, `codegen-units = 1`,
  `opt-level = 3`, `strip = true`, `panic = "abort"`. Do not change.

## Version & Update Command

All `-V` / `--version` output MUST follow this exact format:

```
Version: v0.1.0
Build: linux-x86_64 (11114e7)
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0-or-later
Source: https://github.com/oxyzenQ/zylaxion
```

A `--check-update` subcommand MUST be implemented to check the latest
upstream GitHub release.

## Local Tooling

- **`scripts/build.sh --check-all`:** Local CI gatekeeper. Runs `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test` sequentially. Exit on first failure.
- **`scripts/version-to vMAJOR.MINOR.PATCH`:** Single-source-of-truth version bumper.
  Patches all `Cargo.toml` files. Zero tolerance for manual version edits.

## Build Prerequisites

Zylaxion plays audio through `cpal`, which talks to the OS audio server
(PipeWire / PulseAudio / ALSA) via the ALSA backend. Only the ALSA
development headers are required at build time; no PipeWire native
headers are needed.

CI installs them in `.github/workflows/ci.yml` via `apt-get install`:

- `pkg-config` — needed to locate `alsa.pc`, `libudev.pc`, `libinput.pc`.
- `libasound2-dev` — ALSA C headers (consumed by `cpal`'s ALSA backend).
- `libinput-dev`, `libudev-dev` — required by `zylaxion-input` for evdev.

A CI runner missing any of these will fail at `cargo clippy` with
`pkg-config` exiting non-zero.

> **History note:** v2.0.0 briefly introduced native PipeWire integration
> via `pipewire-rs` (which pulled in `bindgen`, `libclang-dev`,
> `libpipewire-0.3-dev`, `libspa-0.2-dev`). This was reverted in v3.0.0
> because the `pipewire-rs` crate is unmaintained and breaks on PipeWire
> 1.0+ systems. The ALSA bridge used since v1.0.x has near-zero overhead
> and is rock-solid.

## systemd User Service (v3.0.0+)

- `assets/zylaxion.service` is a systemd **user** unit (not system —
  Zylaxion runs per-user so it inherits the user's PulseAudio/PipeWire
  cookie and `XDG_RUNTIME_DIR`).
- `scripts/install.sh` deploys the unit to:
  - `~/.config/systemd/user/zylaxion.service` when run as a normal user.
  - `/etc/systemd/user/zylaxion.service` when run as root (system-wide
    default for all users).
- `scripts/uninstall.sh` removes the unit and prints a reminder to run
  `systemctl --user disable zylaxion` first.
- The unit uses `Type=simple`, `Restart=on-failure`, `RestartSec=3`,
  and `After=pipewire.service sound.target` so it always starts after
  the audio stack is ready.

## CI/CD (GitHub Actions)

- **CI paths filter:** Ignore `*.md`, `*.txt`, `docs/` on push/PR.
- **Node.js:** `FORCE_JAVERCRIPT_ACTIONS_TO_NODE24=true` in all workflow env.
- **Dependabot:** REMOVED. Do not use Dependabot.
- **Dependency updates:** Custom `maintenance.yml` workflow auto-updates deps
  and commits directly to `main`. NO PRs. NO branch spam.
- **Maintenance schedule:** Weekly Monday 07:00 UTC.
- **Bot identity:** Commits as `github-actions[bot]`.
- **Workflow title:** MUST be exactly `"Maintenance deps weekly"`.
- **Linting:** Run `actionlint` and `yamllint` on `.github/workflows/*`.

## Branding & Metadata

- **Author:** `rezky_nightky (oxyzenQ)`. No casing inconsistencies.
- **Repository:** `github.com/oxyzenQ/zylaxion` (capital Q).
- **Contact:** `with dot rezky at gmail dot com`.
- **Project name:** `zylaxion` (lowercase). Never `Zylaxion`.
- **Badges:** ONLY Ko-fi (`ko-fi/rezky`).
- **License:** GPL-3.0-or-later.
- **Trademark:** See `docs/trademark.md`.
- **File headers:** All `.rs`, `.sh`, `.yml` files MUST carry:
  ```
  Copyright (C) 2026 rezky_nightky
  SPDX-License-Identifier: GPL-3.0-or-later
  ```

## Git & Repo Hygiene

- `.gitignore`: Keep lean. Ignore `worklog.md`, `codex/`, `agent/`, `agent-ctx/`,
  and other AI tool directories.
- No tracked AI artifacts.

## Configuration System (Zylaxion-Specific)

- A single `config.toml` (repo root, installed to
  `${PREFIX}/share/zylaxion/config.toml`) holds all DSP parameters.
- Search order: `~/.config/zylaxion/config.toml` → `/etc/zylaxion/` →
  `/usr/local/share/zylaxion/` → `./config.toml` → hardcoded default.
- The file uses a `[default]` table plus optional `[[keys]]` per-scancode
  overrides. Both are validated and clamped by `KeyProfile::validate_and_clamp`
  on load — out-of-bounds values (e.g. `decay = 9999`) are silently clamped
  to safe ranges with a `log::warn!`.
- The running daemon polls `config.toml`'s mtime every 1 second (via the
  `config-watcher` thread) and atomically swaps the `AcousticModel` behind
  an `ArcSwap` on change. No restart, no IPC `reload` command.
- `zylaxion testconf` validates the config without starting the engine —
  equivalent to `nginx -t` or `sshd -t`.
- DSP parameters flow: `KeyProfile` → `MechanicalClick::with_overrides()` →
  `VoicePool::trigger()` → `init_state()`. All TPT filter coefficients are
  set from profile values — this chain must never be broken.
