# Changelog

All notable changes to zylaxion.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

This file carries ONLY the entries newer than the v10.2.0 release —
the fresh record for the v11 line. night-improve-5 (2026-10-11)
trimmed the legacy bulk out so this file starts clean for the
upcoming version instead of dragging the archive along: the v10-era
history (the v10.0.0, v10.1.0 and v10.2.0 entries plus the v6.0.1
summary stub, as they stood at the end of the v10 line) is archived
in
[docs/archive/CHANGELOG-V10.0.0-ERA.md](docs/archive/CHANGELOG-V10.0.0-ERA.md),
and the pre-v10 release notes live next to it in
[docs/archive/CHANGELOG-v6.0.0.md](docs/archive/CHANGELOG-v6.0.0.md).

## [Unreleased]

### Changed

- **night-improve-3**: the README grows a Demo section in the zelynic
  shape — the brand-styled `zylaxion -h` output rendered as
  `assets/zylaxion-help-demo.png` inside the round-border wrapper
  (border-radius 24px + soft shadow, purple-family branding). The
  asset is an honest capture: the image is typeset from the actual
  binary's stdout (color escape stream parsed verbatim), not a mock —
  and the help surface touches no audio or input hardware, so a
  capture host renders it byte-identical to a desktop. A demo video
  (typing sounds) requires real audio hardware the sandbox lacks; the
  README records that it follows at the next owner-host capture
  session, the same standing-disclaimer class the zelynic demo
  section uses.

- **night-improve-1**: the scripts tree now follows the zelynic
  organization — `scripts/package/install.sh`,
  `scripts/package/uninstall.sh`, and `scripts/dev/version-to.sh`
  join `scripts/build.sh` at the root, matching the zelynic layout
  (`dev/` for development tooling, `package/` for install flow). All
  internal and documentation references updated (README, RULES.md,
  and the scripts' own usage strings) and `install.sh`'s repo-root
  resolution adjusted for its new depth. `build.sh` gains the zelynic
  gate interface: the bare `check-all` spelling plus `-q`/`--quiet`
  (essentials-only output — verdicts always emit, banners do not), so
  `./scripts/build.sh check-all -q` works verbatim. File permissions
  normalized to the owner's 644/755 contract (the zelynic
  check-permissions policy): the 21 non-executable files tracked as
  100755 (sources, manifests, lock, license, gitignore) demoted to
  644, the four shebang scripts stay 755, directories 755.

- **night-improve-2**: the helper output (`-h`/`--help`, usage lines,
  and clap-printed errors) now carries the brand palette via a new
  `cli_styles` module — the zelynic clap-styling pattern. Section
  headings (`Usage:`, `Commands:`, `Options:`) render bold brand
  purple #A855F7; every flag spelling (`-v`, `--verbose`,
  `--preset <PRESET>`) and every command synopsis rides the calm grey
  grammar tier #8B8B8B (flags are typeable grammar, not content — the
  owner's eye-strain call); error labels render bold error red
  #FF5A5A, typed mistakes invalid yellow #FFEB3C, and did-you-mean
  candidates the same calm grey. Help prose tidied alongside: the
  `--live` doc comment split into short and long help (the summary
  view shows one line per flag again), the `testconf -f` doc gained
  an explicit paragraph break, and the `start` about dropped its
  trailing period for synopsis consistency. Gatekeeper green: fmt +
  clippy + 111 tests.

- **night-improve-4**: the workspace rides Rust edition 2024 with the
  MSRV pinned to 1.98.1 — `rust-version = "1.98.1"` now sits next to
  `edition = "2024"` in the shared `[workspace.package]` table and is
  inherited by all six crates, and `rust-toolchain.toml` pins the same
  1.98.1 channel in dormant mode (a future stable cannot silently
  break the build). Migration verified clean: the only 2024 hard error
  was `std::env::set_var`/`remove_var` becoming unsafe — the call
  sites gained `unsafe` blocks with SAFETY comments plus a crate-wide
  `ENV_LOCK` so env-mutating tests serialize; four nested guards
  collapsed into 2024 let-chains; `cargo fmt` re-applied the 2024
  style edition (import version-sort, closure indentation). The main
  crate manifest carries the zelynic-grade package data: extended
  description, docs.rs documentation link, readme, keywords, and
  registry categories. Gatekeeper green: fmt + clippy
  (`--all-targets --all-features -D warnings`) + 111 tests pass on
  the pinned 1.98.1 toolchain.

- **night-improve-0**: the design philosophy is now codified in
  `docs/PHILOSOPHY.md` under the owner's "the boring and stealth"
  banner — one sound done perfectly, the keystroke never leaves the
  process, Linux-first evdev-only input, pure procedural synthesis
  with zero assets, lean dependencies, honesty over impression. The
  README Security & Privacy section links to it. Docs-only change;
  no code touched.

- **night-improve-5**: restructured the changelog — the v10-era
  record moved to `docs/archive/CHANGELOG-V10.0.0-ERA.md` (frozen
  history, wrapped with an archive note), the v6.0.0 release notes
  joined it in `docs/archive/`, and this file restarted as the fresh
  record for the upcoming version. No code change; no version change
  (bumps stay the owner's call).

<!-- Entries land here as they merge; the owner promotes
     [Unreleased] to a dated version heading at release time. -->
