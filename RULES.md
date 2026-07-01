# zylaxion — Project Rules

> Absolute source of truth for maintaining consistency, efficiency, and quality.

## Architecture & Code Quality

- **LOC Limit:** Core engine (`zactrix-engine` + `zactrix-profiles`) MUST remain under 1,000 LOC.
  Excludes `*.md`, `*.txt`, `*.toml`, `examples/`.
- **Modular `main.rs`:** `crates/zylaxion/src/main.rs` MUST stay within 100–300 LOC.
  Bootstrap and wiring only.
- **File Bloat:** No single `.rs` file may exceed 800 LOC.
- **Release Profile:** `[profile.release]` uses `lto = "thin"`, `codegen-units = 1`,
  `opt-level = 3`, `strip = true`, `panic = "unwind"`. Do not over-optimize.

## Version & Update Command

All `-V` / `--version` output MUST follow this exact format:

```
Version: vX.Y.Z
Build: linux-x86_64 (git-hash)
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0-only
Source: https://github.com/oxyzenQ/zylaxion
```

A `--check-update` subcommand MUST be implemented.

## Local Tooling

- `scripts/build.sh --check-all` — Local CI gatekeeper: fmt → clippy → test → audit.
- `scripts/version-to.sh vX.Y.Z` — Single source of truth for version bumps.
- `scripts/install.sh` / `scripts/uninstall.sh` — Install/uninstall for Linux.

## CI/CD (GitHub Actions)

- CI paths filter: Ignore `*.md`, `*.txt`, `docs/` on push/PR.
- Node.js: `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` in all workflow env.
- Dependabot: REMOVED.
- Maintenance schedule: Weekly Monday 07:00 UTC.
- Workflow title: `"Maintenance deps weekly"`.
- Linting: Run `actionlint` and `yamllint` on `.github/workflows/*`.

## Branding & Metadata

- **Author:** `rezky_nightky (oxyzenQ)`.
- **Repository:** `github.com/oxyzenQ/zylaxion` (capital Q).
- **Contact:** `with dot rezky at gmail dot com`.
- **Project name:** `zylaxion` (lowercase). Never `Zylaxion`.
- **Badges:** ONLY Ko-fi (`ko-fi/rezky`).
- **License:** GPL-3.0-only.
- **File headers:** All `.rs`, `.sh`, `.yml` files MUST carry:
  `Copyright (C) 2026 rezky_nightky` / `SPDX-License-Identifier: GPL-3.0-only`

## Git & Repo Hygiene

- `.gitignore`: Keep lean. Ignore `worklog.md`, `codex/`, `agent/`, and other AI tool directories.
- No tracked AI artifacts.

## Security & Privacy

Zylaxion has a **zero-leakage** posture:
- Never log scancodes.
- IPC socket MUST be `0o600`.
- No inbound network, no outbound telemetry.
- No on-disk keystroke storage.
