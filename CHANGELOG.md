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

- **night-improve-21** (39586b4): the cosmic dragon unification — the
  six-crate workspace (zactrix-engine, zactrix-profiles, zylaxion-core,
  zylaxion-input, zylaxion-output, zylaxion) becomes ONE package at the
  repo root, the cosmic dragon engine and architecture. The former
  crate boundaries survive as module boundaries: src/engine/ (the
  voice pool), src/profiles/ (the acoustic model), src/core/ (the
  Orchestrator spine, the error contract split into src/core/error/
  under the no-mercy 600-LOC cap), src/input/ (libinput capture),
  src/audio/ (the cpal sink and the SPSC ring bridge), and the shell
  modules (cli, commands, daemon, info, output, session) unchanged.
  The Pattern-C test tree consolidated under the root test/ with every
  #[path] wiring remapped; the five vetted examples/ targets moved
  with module includes and keep their autoexamples-ON status. The
  dependency table is the exact union of the six former manifests,
  every version untouched (11.0.0 lineage, owner-decided bumps);
  cross-crate imports rewired to crate:: paths (the bare `input`
  extern carries a leading :: so the input module and the libinput
  binding never collide). check-loc.sh retires the per-crate loop
  with the workspace; the single-file policy now has one subject. Dead
  re-export bridges retired; unwired API-complete items carry scoped
  allow/expect markers with night-improve-21 reasons. Live docs
  refreshed tree-wide (README, RULES, GLOSSARY, BRANDING,
  CONTRIBUTING, QA, SECURITY, NOTICE, config.toml, release.yml); the
  frozen historical records stay untouched.

- **night-improve-20** (bb23bfb): the lowercase law enforced — the
  zelynic NIGHT-dinner-22 lineage, ported whole. New
  `scripts/gates/check-name-case.py` (gate-keepers section 9) scans
  EVERY tracked file and every tracked path (git ls-files, the .cargo/
  and .github/ hidden trees included, nothing excluded) and fails on
  any casing outside the two legal families: lowercase, and the
  identifier family (ZYLAXION_* env vars and build stamps, plus the
  all-caps banner titles heading the scripts/ gate files). The tree
  re-cased: 44 wrong-case occurrences across 23 files — README, RULES,
  BRANDING, the v6.0.0-era frozen records included (the law excludes
  nothing), the three crate descriptions, the systemd unit
  description, and the instance-lock error string (sentence-start
  lowercase is the law). docs/BRANDING.md grew the full Name Usage
  codification (correct form table, identifier-family rule, incorrect
  forms, the machine-enforcement pointer). 92 files, 671 name tokens,
  0 violations.

- **night-improve-19** (83ea734): the UX bridge — every fatal parse
  error now renders through one authority: brand-styled clap render
  through the shared capability gate (one color law), the
  case-insensitive typo rescue behind clap's own engine, exactly one
  canonical "For more information, try '--help'." footer (clap cannot
  append it — zylaxion's --help is a parsed field per the single-tier
  law), exit 2. The rescue ladder: clap's tip stands; then the
  top-level authority table (help/V/check-update/live typed at
  subcommand position tip the exact top-level spelling); then the
  case-insensitive Jaro fallback over the FAILING command's flag pool
  — argv walks to the subcommand the user is actually in, so
  `zylaxion start --PRESET` rescues `--preset` where the reference's
  root-only pool would stay silent. New `cli/ux.rs` + `cli/suggestion.rs`
  (Levenshtein value matcher, both sides case-folded for the mixed-case
  preset vocabulary — `cherymx` tips `cherryMX` — and the Jaro flag
  matcher); the two duplicated preset-not-found sites deduplicated into
  one constructor carrying the did-you-mean tip. Live-proven on all
  five surfaces; 6 bridge pins, 7 engine pins, 2 config pins.

- **night-improve-18** (4deabb2): the no-mercy era — the zelynic
  structure discipline completed. The LOC gate law rewritten: 600
  lines for every tracked .rs anywhere in the tree, NO marker (the
  `// LOC_EXEMPT:` debt lane retired), over the cap means SPLIT the
  same hour; beside the cap the gate enforces the layout law (every
  crate's src/ root holds exactly its entry file, main.rs / lib.rs,
  everything else in dir/mod.rs subsystem dirs). The Pattern C test
  tree: every *.rs test file lives under its crate's test/ directory
  (19 files, #[path]-wired, one theme per file) — module positions
  unchanged so super::* still resolves, 150 tests, the exact
  pre-move count. The six-marker debt ledger is PAID (max file 587):
  zactrix-profiles/src/lib.rs (1641) into params/, model/, profile/,
  overrides/ + a wiring-only lib.rs with crate-root re-exports;
  mechanical.rs (1337) into mechanical/{mod,render}.rs; config.rs
  (980) into config/{mod,paths}.rs; zylaxion-core/lib.rs (981) to 587;
  zactrix-engine pool.rs (918) to 297 + voice/ dir; daemon.rs (679)
  into daemon/{mod,watcher}.rs. Docs refreshed (RULES.md's stale
  800-LOC/2000-LOC claims replaced with the live law, QA.md Q4, the
  glossary marker entry retired, MAINTENANCE ledger 1 paid).

- **night-improve-17** (58331f1): the masterclass README — the zelynic
  consistency pass. Stability section: the Stable API contract (from
  v11.0.0, the CLI surface frozen — commands, flags, output format)
  plus the release-channels law (dev/nightly/alpha/beta/rc, CI rejects
  any other suffix). Support section: Ko-fi plus the crypto-donations
  lane with the three owner-verified receive addresses and the
  network-mismatch warning. Commercial Licensing section: the ladder
  at the owner's prices — Business $2,199/year (individual businesses
  and small teams), Company $9,199/year (companies and enterprise, or
  redistribution rights); Personal free, Individual holds at
  $99/year — with the rationale (a masterpiece whose value increases
  year by year; the price tracks the value; grandfathering protects
  earlier buyers) and the unauthorized-use warning in the owner's own
  wording: stealing, illegal forking, or building closed-source
  commercial products without first contacting the owner
  (rezky_nightky / oxyzenQ) and paying the license is the mark of the
  worst kind of human — a rotten one; the honest paths are exactly
  two. COMMERCIAL_LICENSE.md re-priced with the three owner-verified
  QR codes (assets/qr/, the same verified set) beside their payment
  addresses.

- **night-improve-16** (9c61e03): the privilege guard and the root-usage
  audit — the zelynic NIGHT-hunt-11 contract lands verbatim:
  `--check-update` under euid 0 refuses with the "do not use sudo" verdict
  and a re-run-without-sudo tip BEFORE any network I/O (exit 1). The total
  audit (the verdict table lives in the new module's docs): `start` /
  `daemon` refuse too, before the instance lock and before the fork —
  acquiring the lock as root IS the pollution event (root-owned
  lock/socket/PID files inside the user's runtime dir break the next
  user-mode run), and a root daemon grabs input system-wide in a session
  no user desktop shares. `doctor` stays reachable but warns its verdicts
  describe root's session. `stop` / `status` deliberately unguarded (the
  rescue path). The new `session::privilege` module (after the
  night-improve-14 folders) carries pure, unit-pinned refusal wordings
  (4 drift-pin tests) and `euid_is_root()` via nix.

- **night-improve-15** (8c87d18): the single help reference and the
  verbose verdict — the owner's suspicion audited and confirmed:
  `-v/--verbose` was a gimmick (clap field parsed and never consumed, the
  real work an argv peek setting RUST_LOG=debug through an unsafe
  set_var, and only 3 debug!/trace! sites in the whole engine), so it is
  removed, not imitated; `cmd_start` logs at info directly and RUST_LOG
  stays the power-user door. The help surface takes the zelynic
  single-tier law: clap's auto help and `help` subcommand disabled, one
  curated end-to-end reference (purple group headings, grey synopses and
  flag spellings, green annotated example pairs) dispatched from a real
  `-h/--help` field; subcommand-level `--help` is now an
  unknown-argument error (exit 2) and `start --check-update` no longer
  hijacks into an update check. Three drift-pin tests hold the reference
  to the command surface.

- **night-improve-14** (5f6e40a): the zelynic foundation shape — src/
  root holds exactly one file (`main.rs`, bootstrap + dispatch; the
  audit found ten loose files there). Every concern moved to its folder:
  `cli/` (Cli + Commands + version report + clap styles), `output/`
  (the color/lane system, the former error_format.rs), `info/` (build
  facts + the Hinnant clock), `session/` (config, instance lock,
  pathguard, privilege, signals — the session contract); `commands/` and
  `daemon/` unchanged. About 94 module-path references rewritten, the
  `include_str!` manifest depth re-pinned, and the docs (QA.md,
  BRANDING.md) re-pointed at the new homes.

- **night-improve-12** (the mature-infra depth audit, five landing
  commits): the zelynic estate ported and adapted —
  - **12a (06b0ea6)**: the dual-license estate — COMMERCIAL_LICENSE.md
    (the four-tier ladder at the owner's current prices, the
    owner-verified crypto addresses), NOTICE, SECURITY.md (the
    input-data discipline: no key content persists, no network in the
    audio path, a 0o600 IPC socket speaking only documented verbs),
    CLA.md, CONTRIBUTING.md, and the QA record seeded with
    session-verified facts.
  - **12b (c10e9c1)**: the gate-keepers — six gates under scripts/gates/
    (check-loc 600-line law with the honest LOC_EXEMPT debt ledger,
    check-scripts-loc 1000-line cap, check-permissions the exact
    644/755 contract with shebang parity, check-headers, the
    check-language English-only sweep with the NON_LATIN_FIXTURE
    marker, the inline emoji sweep) plus gate-keepers.sh, every section
    a hard gate. The cleanups the gates demanded: 16 files gained the
    license header, the doctor switched to word verdicts (PASS:/FAIL:),
    the ohm unit ASCII-ized, the OS line added to the pre-existing
    scripts. The permission gate proved itself on run one: the umask-002
    worktree drift across 70+ files caught and --fix restored.
  - **12c (fd585ef)**: the arch-baseline release lane — Dragon Guard -
    Release with the channel/version contracts, the x86-64-v3/v4 gnu
    matrix, embedded-label verification (v3 executed, v4 byte-grepped),
    the flat tarball invariant, three checksum families (SHA-512,
    BLAKE2b-512, SHAKE256 quantum-resistant), the GPG detached-
    signature lane with passphrase confinement, the dist/ inventory
    tripwire; the pro-linux-amd64-v3-gnu / v4-gnu aliases + profiles
    with live-proofed local builds, and the release-parity gate
    (section 8). The musl legs are a documented dormant lane with the
    session's empirical evidence: static musl alsa-lib proven buildable
    end to end, the real blocker libudev-sys/libinput, unlock paths
    recorded for the owner's call.
  - **12d (ee851c8)**: the miri UB lane (Dragon Guard - Miri — the
    workspace suite under the nightly interpreter, weekly Sunday plus
    path filters) and the family rename — every workflow now carries
    the Dragon Guard name.
  - **12e (1fd74f5)**: the docs structure — the map (docs/README.md),
    the glossary (docs/GLOSSARY.md), the maintainer handbook
    (docs/MAINTENANCE.md).

- **night-improve-13** (0fd7cc4): the Hinnant clock and the zelynic
  version-report contract — env_logger stripped to `auto-color` (its
  `humantime` feature has mapped to dep:jiff since 0.11.8, and the
  default regex filter chain went with it): eight crates left the lock
  outright (jiff ×3, defmt ×3, thiserror ×2) plus the regex tree, and
  zylaxion now carries no time crate and no regex crate anywhere. The
  wall-clock needs answer through Howard Hinnant's civil_from_days
  instead: the new clock.rs renders the daemon log stamps (same
  millisecond UTC shape, journal greps keep their contract), build.rs
  stamps `ZYLAXION_BUILD_TIME` plus the rustc/profile/opt-level facts,
  and the new build_info lane feeds the version report and the doctor.
  `-V`/`--version` now speaks the zelynic report shape — brand header
  (purple on a TTY via the new stdout lanes), `Build:` /
  `Build-time:` / `Copyright:` / `License:` / `Source:` — with one
  body shared between the first-arg intercept and clap's own path; the
  doctor leads with the `[1/4] Build facts` block (Version, Commit,
  Variant, Built, Rustc, Profile with the manifest drift-guard test).

- **night-improve-11**: cargo aliases land in the zelynic usage shape —
  `.cargo/config.toml` (now tracked; the local-only ignore rule retired)
  carries `cargo pro-native-gnu` (host-CPU native build via its own profile)
  and `cargo pro-linux-amd64-gnu` (the exact release workflow contract).
  Honest deltas documented in place: no musl blocks (the ALSA constraint)
  and no build-level rustflags (release already pins codegen-units).

- **night-improve-10**: masterclass package data — the workspace author row
  adopts the zelynic obfuscated-contact format, and all six crates carry
  explicit auto-discovery controls (a stray tests/, benches/ or examples/
  directory can never silently become an unvetted target; the four crates
  with real examples/ keep that discovery ON deliberately).

- **night-improve-9**: `version-to.sh` inherits the zelynic abilities — a
  changelog release ledger (a bare version line under `## [Released]`,
  newest first, probed honestly), a sibling-manifest bump lane for any
  future own-version crate, and a six-member Cargo.lock sync (cargo
  update --workspace --offline, verified per member, third-party deps
  frozen). Pre-release versions validate now.

- **night-improve-8**: the actions-pin health gate — all eight workflow
  pins moved from floating tags to release-commit SHA pins with version
  tokens (the same commits the zelynic estate runs), and
  `scripts/gates/check-actions-pins.sh` keeps them current: modes
  off|warn|strict|auto, content-hash cache with TTL, honest skips, MAJOR
  moves never auto-healed, guarded apply.

- **night-improve-7**: the Supermassive deep battery — weekly plus on
  demand, two legs on the pinned toolchain: the full test battery (all
  targets, all features) with the docs build warnings-denied, and the
  release shape proof (the night-improve-11 alias plus the binary's
  hardware-free surfaces). The header states the physics honestly: no
  audio or input device in CI, no claims about real ALSA timing or
  libinput event flow.

- **night-improve-6**: the labeled color lanes — the depth audit of every
  error/failed/unknown/typo path. `error_format` becomes the tier module
  (bold red error lane, bold yellow warning lane, calm grey passive
  suggestions, status green runnable tips, brand purple headers) behind
  one capability gate (TTY + NO_COLOR + CLICOLOR), broken-pipe-safe, with
  the mono prefix byte-identical to the grep-able historical shape. The
  stray hand-rolled error prints in daemon/mod.rs, update.rs and
  instance_lock.rs now ride the labeled lane, and BRANDING.md documents
  the palette.

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
