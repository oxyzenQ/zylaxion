# Changelog

All notable changes to zylaxion.

## [v10.2.0] — 2026-07-12

### Dragonzen Depth Audit — 49 items fixed, 3 won't-fix, 20 new tests

Comprehensive depth audit covering naturalness, stability, bugs,
inconsistencies, resource efficiency, and test coverage. All changes
backward-compatible unless explicitly noted.

#### Critical (3)
- **B1**: `voice_off_threshold` in `technical` preset was assigned to
  `[preset.technical.ambient]` instead of `[preset.technical.decay]`
  due to TOML table scoping. Moved under the correct table.
- **B2**: `ProfileWithOverrides::parse` silently dropped `[[keys]].ambient.*`
  overrides — public API was inconsistent with the daemon path. Fixed.
- **I1**: `assets/zylaxion.service` had `GPL-3.0-or-later` instead of
  `GPL-3.0-only`. Fixed per RULES.md.

#### High — Naturalness (5)
- **N1**: Per-keystroke noise seed via splitmix64 + 6 xorshift32 rolls.
  Fixes "metallic ringing" on autorepeat (Backspace held at ~30 Hz).
- **N2**: Soft release ramp (~2 ms) replaces hard-cut `active=false` on
  key-up. Eliminates the "click off" tell.
- **N3**: Housing noise stream decoupled from click noise stream.
  Eliminates constructive interference that merged click + thock into
  a single "honk".
- **N8/B14**: `u32_to_signed_f32` precision + bias fix (upper 24 bits,
  symmetric range). Deduplicated across all 3 noise paths.
- **B18**: Dropped unnecessary `.abs()` on strictly-positive envelope.

#### High — Bugs (10)
- **B3**: libinput loop rewritten to `poll(2)` on fd. Zero idle CPU,
  instant wake on event. Was `thread::sleep(1ms)` busy-yield.
- **B4**: `is_daemon_running` uses `/proc/pid/exe` readlink instead of
  `/proc/pid/comm` exact match. Handles renamed binaries + symlinks.
- **B5**: `fade_out_before_drop` paces silence writes against
  `producer_vacancy()` instead of blind tight-loop push.
- **B6**: `spawn_ipc_thread` + `spawn_config_watcher` return `Result`
  instead of `.expect()` panicking.
- **B7**: `close_std_fds` explicitly closes the `/dev/null` fd after
  `dup2` — was leaked for daemon lifetime.
- **B8**: i16 audio path uses `.round()` before cast — eliminates DC
  offset from truncation-toward-zero.
- **B9**: `daemonize()` uses pipe-based init sync. Parent blocks on
  child init completion — no more silent daemon death.
- **B10**: IPC listener set to non-blocking + `stop_flag` poll (100ms).
  Thread exits within 100ms of shutdown.
- **P1**: `master_volume` configurable via `[master]` table in
  `config.toml`. Was hardcoded 5.5x (ear-damaging for headphones).
- **S1**: Config-watcher thread accepts `stop_flag` + exits cleanly.
  Was leaking at process exit.

#### High — Stability (1)
- **S2**: IPC thread wraps each connection in `catch_unwind`. Panic
  doesn't kill the thread — daemon stays responsive to `stop`.

#### Medium — Naturalness (4)
- **N4**: Quadratic fade for click path, exponential taper for housing
  path. Was linear — produced perceivable "tick" at burst boundary.
- **N5**: Inter-keystroke timing variation. Fast repeats (≤80ms)
  attenuated up to -5%. New `AcousticModel::record_trigger_timestamp`
  trait method.
- **N6**: Per-keypress stereo pan jitter (±3%). Unlocks the locked
  stereo field — the most obvious headphone tell.
- **N7**: Two-stage decay envelope (`coefficient_fast` +
  `fast_samples_ms`). Opt-in, backward-compatible.

#### Low — Cleanup (19)
- **I2**: Script headers normalized to `Copyright (C) 2026 rezky_nightky`.
- **I3**: `DEFAULT_PRESET` documentation clarified (last-resort fallback
  vs shipping default).
- **I6**: `RUST_LOG` set before `Cli::parse()`. Pre-existing env var
  takes precedence.
- **I7**: Legacy `load_profile_from_*` marked `#[deprecated]`.
- **I8**: RULES.md LOC limit relaxed 1,000 → 2,000 (reflect reality).
- **I9**: `run_check_update` extracted to `commands/update.rs`.
  main.rs 273 → 68 LOC.
- **I10**: RULES.md typo `JAVERCRIPT` → `JAVASCRIPT`.
- **P2**: `$XDG_CONFIG_HOME` honored per XDG Base Directory spec.
- **P3**: Config-watcher requires mtime + size change (chmod no longer
  triggers reload).
- **P4**: `Voice.scancode` zeroed on release (privacy).
- **P5**: `install.sh` allows root for `--system` mode (Docker/CI).
- **P6**: `validate_config_str` misleading comment fixed.
- **B11**: SIGHUP added to graceful-shutdown set.
- **B12**: Audio device fallback via `output_devices().next()`.
- **B15**: Warning log if device reports >2 channels.
- **B16**: Nav cluster (scancodes 70-89) panned center-right.
- **E3**: Orchestrator sleeps 500us on full buffer instead of busy-loop.
- **S3**: Panic hook logs to journald (silent-audio-death debuggable).
- **S4**: Heartbeat log: debug! at 60s, warn! at 5min of input inactivity.
- **I5**: CI `audit` job runs `cargo audit --deny warnings`.

#### Pre-v11.0.0 Priorities (3)
- **P7**: 7 IPC layer unit tests (stop/status/unknown/malformed/empty +
  serde round-trip).
- **P8**: 6 orchestrator integration tests via `MockSink` + generic
  `Orchestrator<S: AudioSink>`. Tests: trigger, decay, stop, disconnect,
  hot-reload, master_volume.
- **B17**: Suspend/resume recovery. After 50 consecutive dispatch
  errors (≈5.5s), re-calls `udev_assign_seat()` to re-enumerate devices.

#### Final (2)
- **B13**: `sample_rate` is now `Arc<AtomicU32>` shared between
  orchestrator and watcher. Watcher reads current value before each
  reload instead of stale captured value.
- **I4**: `zactrix_profiles::KeyEvent` renamed to `KeyTrigger`.
  Eliminates naming collision with `zylaxion_input::KeyEvent`.

#### Won't-fix (3 — documented rationale)
- **E2** (inotify config watcher): 1 Hz poll cost is ~5us — invisible.
  Adding `notify` crate violates lean-dependency philosophy.
- **E4** (VoicePool 16-voice scan): N=16 is too small to matter.
  3.5ms CPU/sec. Adding index tracking adds complexity for no gain.
- **E5** (scancode_to_pan per keypress): 10ns per keypress. 10us/sec.
  A const lookup table would save 8ns. Not worth the complexity.

#### Tests
- 90 → 110 (+20 new: IPC 7, orchestrator 6, DSP 4, release-ramp 2,
  B2 regression 1).
- Gatekeeper: `./scripts/build.sh --check-all` green (fmt + clippy +
  test). cargo-audit in CI.

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
