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

A `--check-updated` subcommand MUST be implemented to check the latest
upstream GitHub release.

## Local Tooling

- **`scripts/build.sh --check-all`:** Local CI gatekeeper. Runs `cargo fmt --check`,
  `cargo clippy -- -D warnings`, `cargo test` sequentially. Exit on first failure.
- **`scripts/version-to vMAJOR.MINOR.PATCH`:** Single-source-of-truth version bumper.
  Patches all `Cargo.toml` files. Zero tolerance for manual version edits.

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

## Profile System (Zylaxion-Specific)

- Profiles are TOML files in `profiles/` (development), installed to
  `${PREFIX}/share/zylaxion/profiles/`.
- Search order: `~/.config/zylaxion/profiles/` → `/usr/local/share/` →
  `/usr/share/` → `./profiles/` → hardcoded default.
- DSP parameters flow: `KeyProfile` → `MechanicalClick::with_profile()` →
  `VoicePool::trigger()` → `init_state()`. All TPT filter coefficients are
  set from profile values — this chain must never be broken.
