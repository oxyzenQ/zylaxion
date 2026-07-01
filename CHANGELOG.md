# Changelog

All notable changes to zylaxion.

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
