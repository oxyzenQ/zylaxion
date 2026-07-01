# Changelog

All notable changes to zylaxion.

## [v10.0.0] — 2026-07-01

### Architecture Alignment

### Fixed — Release YAML
- Fixed `body:` indentation (was outside `with:` block — caused actionlint failure)
- Fixed SPDX header: GPL-3.0-or-later → GPL-3.0-only
- Removed duplicate closing block in release body
- Release body: polished, simple, not boring

### Changed — License Consistency
- Workspace Cargo.toml: GPL-3.0-only (was GPL-3.0-or-later)
- All crates inherit workspace license

### Verified
- 81 tests PASS (where buildable — libudev-dev required for full build)
- codespell PASS, yamllint PASS, actionlint PASS

## [v6.0.1] — Previous release

- Real-time mechanical keyboard acoustic synthesizer
- 6 workspace crates (engine, profiles, output, input, core, binary)
- 7,564 LOC, 81 tests
