# Changelog

All notable changes to zylaxion.

## [v10.1.0] — 2026-07-09

### Security — Pathguard for Runtime State I/O

### Added — `crates/zylaxion/src/pathguard.rs`
- New module validates `$XDG_RUNTIME_DIR` before use for lock/socket/PID
  files. Falls back to `/tmp` when the env var points to a dangerous
  system path (e.g. `/etc`, `/usr`, `/var`, `~/.ssh`, `~/.gnupg`).
- `is_dangerous()` rejects system path prefixes (`/etc`, `/usr`, `/var`,
  `/bin`, `/sbin`, `/lib`, `/lib64`, `/boot`, `/root`, `/proc`, `/sys`,
  `/dev`) and user credential paths (`~/.ssh`, `~/.gnupg`, `~/.kwallet`,
  `~/.local/share/keyrings`) — matches exact path OR path + `/` + anything.
- `resolve_runtime_dir()` returns the validated dir, falling back to
  `/tmp` when dangerous. Path traversal (`/tmp/../etc`) defeated via
  lexical normalization.
- Wired into `daemon/ipc.rs::socket_path()`, `daemon/ipc.rs::pid_path()`,
  and `instance_lock.rs::lock_path()`.
- Config reads from `/etc/zylaxion/` and `/usr/local/share/zylaxion/`
  are BY DESIGN (system-wide config, read-only) — NOT gated here.
- 8 unit tests (CI green).

### Verified — Real Machine Test
- `XDG_RUNTIME_DIR=/etc zylaxion daemon` → PID and socket files landed
  in `/tmp`, NOT `/etc`. Pathguard redirected successfully.
- `XDG_RUNTIME_DIR=~/.ssh` → blocked, redirected to `/tmp`.
- `XDG_RUNTIME_DIR=/tmp/../etc` (path traversal) → blocked, redirected
  to `/tmp`.

### Cleanup — Remove Future Roadmap
- Deleted `docs/ROADMAP.md` (72 lines of unreleased future plans:
  Phase 1-3 covering v10.1.0 polish, v10.2.0 performance, v11.0.0
  ecosystem including Prometheus metrics, D-Bus interface, community
  profile repository, SIMD optimization, etc.)
- `docs/audit-v6.0.0.md` roadmap section left intact (historical record
  of what WAS done for v6.0.0, not future plans).

## [v10.0.0] — 2026-07-01

### Architecture Alignment + Full GPL-3.0-only

### Fixed — License Consistency (CRITICAL)
- Purged ALL `GPL-3.0-or-later` references across entire codebase
- All source files: SPDX header `GPL-3.0-or-later` → `GPL-3.0-only`
- All scripts: `GPL-3.0-or-later` → `GPL-3.0-only`
- README.md, TRADEMARK.md, docs/trademark.md, RULES.md, docs/RULES.md, BRANDING.md
- Zero `or-later` remnants remaining

### Fixed — Release YAML
- `body:` was outside `with:` block (caused actionlint failure)
- Removed duplicate closing block
- SPDX header: `GPL-3.0-or-later` → `GPL-3.0-only`

### Fixed — Cargo.lock
- Updated all 6 workspace crates from v6.0.1 → v10.0.0

### Verified
- fmt PASS, codespell PASS, yamllint PASS, actionlint PASS
- 0 production unwraps outside compile-time guarantees
- 0 GPL-3.0-or-later remnants
- 0 MIT remnants
- CI builds on GitHub Actions (has libudev-dev)

## [v6.0.1] — Previous release

- Real-time mechanical keyboard acoustic synthesizer
- 6 workspace crates (engine, profiles, output, input, core, binary)
- 7,564 LOC, 81 tests
