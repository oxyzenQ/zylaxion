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

### Fixed

- **night-hunt-1** — the 300-second idle warning spoke for a dead
  architecture. The heartbeat that fires after five minutes without
  key events read "input thread may be stuck — restart it
  (`systemctl --user restart zylaxion`)": every noun in that sentence
  is a fossil. The libinput input thread was deleted with the daemon
  (night-improve-23) — input now arrives from the kernel-side tap
  through the pinned ring buffer, and a wedged input lane cannot hide
  inside a running player (the bridge thread dies loudly: the channel
  disconnects and the render loop exits; its poll errors log their
  own warn). The advice named a systemd user unit the installer no
  longer ships, and the message contradicted its own 60-second debug
  sibling, which already admits idle is normal — a user reading or
  watching for five minutes got a restart order for a healthy player.
  The rewritten messages claim only what this seat can honestly know
  (no keys arrived — idle, or the kernel tap is not delivering) and
  route the real failure to the lanes that can distinguish the two:
  `zylaxion doctor` for the environment verdicts and
  `sudo zylaxion attach` to re-plant a detached tap. The same hunt
  caught the last stale restart instruction in the preset-resolution
  module doc, which described the deleted daemon and a user unit —
  and denied the live preset swap the control-map watcher has applied
  since the player landed. The stale-text sweep found no other
  copies: README, QA, docs and the archive changelog never quoted
  the old wording.

- **night-improve-26** — the parent side of the player wire speaks
  honestly and owns its door. The night's audit found the clap side
  already soldered (the explicit `name = "_play"` pin and its parse
  tests landed with night-improve-25), but two parent-side gaps
  remained. First, both startup-failure strings led with
  `zylaxion _play` — pointing a stranded user at a hidden internal
  verb as the primary diagnostic; they now lead with `zylaxion
  doctor` (the environment verdicts) and mention the internal verb
  only as a secondary, honest pointer to something that exists.
  Second, `zylaxion on` had no root door: root sailed past the
  parent into the player's refusal and met a spawn error instead of
  the session wording. `cmd_on` now refuses root BEFORE any path
  resolution, map open, or spawn, through the pure decision surface
  `on_root_refusal` — the exact mirror of the player-side guard,
  one wording, two doors. The wire is pinned against drift besides:
  the spawn literal becomes the `PLAYER_SPAWN_VERB` constant the
  spawn consumes, and test/commands/sound_tests.rs holds the three
  decision-surface pins (the underscore spelling, the root refusal
  carrying the do-not-use-sudo note, the non-root pass).

- **night-improve-26** — the attach cleanup owns the directory. The
  old cleanup unpinned files but never removed the pin directory:
  an attach that failed after `mkdir` left /sys/fs/bpf/zylaxion
  behind, and the next first-ever attach died with EEXIST at that
  path — on CachyOS (nested bpffs) it surfaced on the owner's live
  host (docs/KERNEL_COMPATIBILITY.md carries the entry). The one
  rmdir that did exist was a silent `let _ =`: every outcome —
  foreign entries and real failures included — vanished without a
  word, and the attach-side self-heal fired only when a pin (not
  the bare directory) was present, so a stale-EMPTY directory
  survived into the next attach. The cleanup is now classified end
  to end: `PinDirRemoval` splits the outcomes (Removed,
  AlreadyAbsent, LeftForeignEntries — ENOTEMPTY reads as
  handled-not-fatal because foreign entries are not ours to remove
  — and Unexpected, which surfaces), attach pre-clean removes the
  stale-empty directory before mkdir and refuses honestly on
  foreign entries, detach reports every outcome instead of
  swallowing it, and the failure marker the doctor reads survives
  the cleanup lanes — read, logged, then deliberately superseded.
  The detach half lives in the new teardown module (loader.rs hit
  the 600-line no-mercy cap and split the theme out); the
  classified core is pinned by four fixture tests in
  test/ebpf/pin_tests.rs.

- **night-improve-25** — the hidden player verb parses again: clap
  derives `play` from the bare `Commands::Play` variant, but every
  call site spells the verb with the underscore (sound.rs spawns
  `.arg("_play")`, both startup-error strings teach `zylaxion _play`,
  player.rs's module doc names it, README's architecture diagram shows
  it — eight references to `_play`, zero to `play`). Before the
  explicit `name = "_play"` pin the spawn argument was a verb clap
  rejects, so `zylaxion on` could never start the player — masked
  until now because the broken attach path (the tracepoint fiction,
  fixed earlier this night) sat upstream of the spawn and the
  failure never got reached. The parse pin in test/cli/ux_tests.rs
  holds both directions: the spawned spelling parses and dispatches,
  and the derived bare spelling stays an unknown subcommand.

- **night-improve-25** — the input tap returns to the plan the
  night-private research doc already approved: a passive kernel-side
  probe on the exported `input_event()` function. The
  night-improve-23 attach path targeted a tracepoint that does not
  exist in mainline Linux; the research doc's fallback claim was
  never verified against kernel source. The primary plan (fentry /
  kprobe on the exported input_event function) was correct and is
  what this revision restores. The evidence chain, for the record:
  there is no `include/trace/events/input.h` at any tag from v6.1
  through master, `TRACE_SYSTEM(input)` matches nothing in the
  tree, `drivers/input/input.c` and `evdev.c` carry zero trace
  calls, and the one patch that ever proposed the tracepoint (the
  WangYuli `input-event` series) never merged. The discovery vector
  was the night-badas-1 polarity fix: once `sudo zylaxion attach`
  could actually reach its lane, every attach on every kernel died
  at the tracepoint-id read — the loader's own error text naming a
  hook the code had invented. The four teaching surfaces that carried the
  fiction (README feature bullet + privacy claim + architecture
  diagram, SECURITY.md, the boot oneshot comment, the doctor's tap
  line) are swept with it; `grep -rn 'input_event tracepoint'`
  now returns zero production hits, and the one place the string
  may appear is the negative help pin that fails the suite if it
  ever teaches the removed wrong path again. One honest correction
  to the record: the research doc's "(5.2+) fallback" claim was
  never verified against mainline kernel source and is corrected
  here — the primary plan was always right.
- **night-badas-1** — the estate green on push and schedule, and the
  one privileged surface actually usable:
  - the `Workflow lint + codespell` lane: the release GPG step's
    `mkdir -p -m 700` split into `mkdir -p` + `chmod 700` (SC2174 —
    with `-p`, `-m` only applies to the deepest directory;
    byte-identical outcome, shellcheck-clean).
  - the `Miri` lane: `tee target/miri-log.txt` opened its output
    before cargo materialized `target/` on a cold cache, so the job
    went red with zero failing tests (143/143 interpreted clean) and
    the failure artifact never existed. A `mkdir -p target` ahead of
    the pipeline returns the exit code to cargo and makes the
    artifact contract honest. The bug was masked until
    night-improve-24 landed the eBPF bootstrap the same job needed.
  - `sudo zylaxion attach` refused root: the privilege gate was
    wired with the negated euid fact — root got "attach must run as
    root" while non-root sailed through into the loader's failure.
    The polarity is corrected to match the sibling gates, the doc
    that described the negated call as the contract is fixed, and
    `attach_requirement` gains its missing test pins (the non-root
    refusal wording, and the root-opens-the-gate polarity).
  - the Supermassive and Dependency Maintenance lanes: both were fed
    the eBPF source-build prerequisites (the canonical bootstrap
    behind the shared `ebpf/target` cache), and the daemon-era
    `libinput-dev` / `libudev-dev` packages leave with them.
  - the Release lane latent gap: the build job never had the eBPF
    prerequisites either — the first tag push after night-improve-23
    would have died in the build script's preflight (the last green
    release, v11.0.0, predates the turn by 70 commits). The same
    bootstrap pair rides the matrix now.
  - `scripts/dev/bootstrap-ebpf.sh` header comment: it still claimed
    the default stable build never needs the prerequisites; since
    night-improve-23 every git-checkout build hard-requires them, and
    the comment now says so.
- **night-improve-24** — the Dragon Guard estate is green again. All
  four red lanes diagnosed on real ground and repaired:
  - the `check` and `Miri` lanes died inside the build script's eBPF
    preflight: the ebpf feature went default in night-improve-23, so
    every git-checkout build source-builds the kernel object and
    hard-requires the dated nightly pin + bpf-linker — neither lane
    had them. Both now run `scripts/dev/bootstrap-ebpf.sh` (the
    canonical installer) behind a shared `ebpf/target` cache keyed on
    the toolchain pin, and drop the daemon-era `libinput-dev` /
    `libudev-dev` packages (only the cpal ALSA headers remain a build
    prerequisite).
  - the `Workflow lint + codespell` lane: eight codespell dictionary
    hits repaired or allow-listed (`retuned` is a false positive on
    the deliberate past tense of retune), yamllint comment-spacing
    compliance across all five workflows, and three release.yml
    shell lines back under the 120-character cap with byte-identical
    behavior.
  - the `Supply-chain CVE scan` lane: RUSTSEC-2026-0293 (ringbuf
    0.4.8 double free / use-after-free in `Consumer::skip`/`clear` on
    a panicking `Drop`). The audio bridge never calls either
    function, so the exposure was the advisory, not an exploitable
    path; ringbuf is bumped to 0.5.3 (>= the 0.5.2 fix), an
    API-compatible move for every call site.
  - `scripts/dev/bootstrap-ebpf.sh`: the python zstandard fallback
    installed the raw tar as the linker binary (the release asset is
    a zstd stream wrapping a POSIX tar; only the `tar --zstd` branch
    unpacked both layers). Reproduced on a zstd-less host; the
    fallback now untars and refuses to stage anything without ELF
    magic.
  - the miri-interpreted suite, once it finally ran, found five real
    test bugs plus one interpreter-physics mismatch, all fixed with
    the evidence in the commit: a tracepoint-id test that read /sys
    (split into the pure path-order + parser contract and the root-run
    IO shell), a pin test statx-ing live bpffs paths (fenced from the
    interpreter lane), an alignment fixture that assumed stack
    alignment by luck (bare `[u8; N]` locals carry alignment 1 — now
    `#[repr(align(8))]`, the exact law the file pins), a wall-clock
    probe (fenced), and a batch-vs-per-sample tolerance that ignored
    the interpreter's non-deterministic libm transcendentals
    (1e-10 -> 1e-4, four orders of magnitude below any real
    divergence, with the tan-on-identical-bits experiment recorded).
- **night-improve-24** — the changes job's detector now matches the
  `.github/` tree too: workflow edits are build surface (the check
  lane carries the eBPF prerequisites) and used to ship unvalidated.

### Changed

- **night-improve-25** — the attach chain is fentry -> kprobe ->
  kprobe-alt, three steps, no tracepoint lane at all. The tap runs
  as two programs over one body in the object (`fentry/input_event`,
  `kprobe/input_event`; the alternate symbol is an attach-time
  parameter on the same kprobe program), the argument extraction
  lives in one shim both crates compile (`src/ebpf/args.rs`, the
  userspace crate owns the file, the ebpf crate includes it via
  `#[path]` — one source of truth, tested where tests can run).
  The loader walks the chain and records the winning step in the
  control map's new TAP_KIND slot; `status` and the doctor report
  it ("tap: attached via fentry:input_event"), and a failed attach
  is a classified error that cleans every pin before returning —
  never a half-attached state. Attach on a complete pin set is now
  the success shape "already attached — nothing to do" (the
  double-attach idempotency contract). The capability probe
  pre-verifies the same chain from /sys facts (BTF token search,
  kprobe blacklist) and renders one of four canonical verdict
  strings the doctor prints verbatim; the doctor's kernel section
  also lost its [2/5]-among-[x/4] numbering bug.
- **night-improve-24** — the CLI surface revision, three owner
  verdicts on the flag grammar:
  - `--live` dies totally. The 2-second alternate-screen refresh
    loop was the wrong shape for a status surface — a signal
    handler, a cursor hider, and a terminal restore protocol
    attached to what should be a static read. Six surfaces carried
    the flag (the Cli field, the dispatch branch, the 52-line live
    loop, the top-level rescue table, the help reference, the
    README usage line); all six are cleaned, and `zylaxion --live`
    now answers with the honest unknown-argument error, exit 2.
  - `stats` and `status` are one verb: `zylaxion status
    [--verbose]`. The verbose flag appends the kernel-side typing
    telemetry (the merged stats surface, the zelynic `-v`
    vocabulary) to the one-screen truth — a static print-once deep
    read, never a live refresh. The telemetry renderer keeps its own
    module per the 600-LOC cap, and a section of a delivered report
    warns where the standalone verb hard-exited (the map was the
    verb's whole purpose then; as a section it is a gap to name).
    The stale-teaching pin is new law: the reference must not teach
    a dead verb or a dead flag, and the typo-rescue test token
    moved from the merged `startt` (a stats typo, now singular-tip)
    to `presets` (rescues both `preset` and `list-presets`, keeping
    the plural-engine contract).
  - `preset --name <NAME>` becomes `preset --switch <NAME>` (short
    `-n` becomes `-s`): the verb switches, the flag names the
    action, not the value's type. The rename hunt also found the
    curated help teaching a dead example — `tpt` was never in the
    registry (the author had reached for the TPT waveshaping
    module's name); every teaching surface now says `--switch
    topre`, which smoke-runs through the registry into the honest
    tap-not-attached verdict instead of a not-found error.

### Added

- **night-master-3** — the supermassive E2E: the whole zero-to-hero
  chain on the KVM micro-VM floor, the zelynic lineage's crown job
  ported to zylaxion's claims. The engine
  (`scripts/supermassive/supermassive-test.py` plus the sm_* module
  family and the bash wrapper) drives two lanes: `--self-test`, the
  rootless lane (the CLI/UX battery — identity, discovery, the fatal
  input matrix with exit-code pins, the tap-less doors, the root-door
  polarity — plus the pure-math proof: zero RIFF/WAVE bytes in the
  binary, zero audio assets in the tree; and the zero-config proof:
  a pristine HOME/XDG run writes nothing anywhere), and `--full`, the
  root lane the guest init calls: environment probes, the tap
  lifecycle (attach with the pin handover to a real user, the
  complete pin layout, the live re-attach refusal, the
  foreign-entry-keeping detach, the idempotent re-detach, five rapid
  attach/detach cycles, the zero-residue proof), and the crown — the
  real-usage chain: a user with no input group and no /dev/input
  access turns the sound on, a uinput virtual keyboard types through
  the kernel's own input path, `status --verbose` counts the presses
  the tap accumulated kernel-side, the PCM state under /proc/asound
  witnesses the synthesizer rendering real frames, live preset and
  volume switches ride the pinned maps as the unprivileged user, `off`
  self-exits the player to a zero-process state, and the detached
  `on` closes the loop at the tap-less door. Every probe lands on one
  ledger that prints a human row and the MASS-RESULT sentinel, one
  final MASS-VERDICT line is the CI relay. `scripts/ci/supermassive-init.sh`
  is the guest's PID 1 (the zelynic kernel-floor contract: the mounts,
  the staged module chain from the guest kernel's own debs —
  snd-dummy is the PCM witness's card, uinput the injector's kernel
  half — the install step landing the payload at /usr/bin the way
  install-on-my-machine.sh does, the engine, the isa-debug-exit).
  `.github/workflows/supermassive.yml` grows the e2e-vm job: the
  release-shape payload built on the runner, the guest kernel
  resolved per leg (the documented 5.15 floor from jammy's pockets,
  the newest kernel the runner's own distro serves), an
  ubuntu:24.04 initramfs rootfs, and a dynamic envelope (every core,
  three quarters of the RAM) — two legs, one battery, the verdict
  grepped off the serial console. The self-test lane ran green in
  the build sandbox (26 probes, 1 honest skip); the VM legs ride CI.

- **night-revert-1** — the crates.io publish lane, mature CI and all.
  The owner's verdict on night-simple-1 landed plainly: removing the
  registry lane was a mistake — crates.io is a fully supported
  distribution channel, and the zelynic family already runs the
  mature reference CI for it. The reversal restores everything the
  removal touched (the `documentation`, `keywords` and `categories`
  manifest fields, the `registry-prebuilt` lane naming in
  build/ebpf.rs with its `cargo install zylaxion` panic remedy, the
  registry-tarball narrative in build.rs, loader.rs and
  bootstrap-ebpf.sh) and then grows what the tree never had: a
  `v*`-tag-triggered `Dragon Guard - crates.io publish` workflow —
  the commit+tag collision gate (scripts/release/wait-for-ci.sh,
  ported from the zelynic reference: it serializes the publish behind
  the ci.yml push-run on the exact tagged SHA, with the infra-failure
  carve-out that buys automatic re-runs for GitHub-side faults and
  never mistakes them for a code verdict), the tag/version match
  check, the CRATES_IO_TOKEN presence check, the already-published
  idempotency probe (a re-pushed tag skips instead of failing), and
  the `--locked` upload whose verification build exercises exactly
  the registry-prebuilt lane a `cargo install zylaxion` consumer
  rides (the packaged tree carries no ebpf/ workspace, so build.rs
  stages the maintainer-built objects from ebpf-prebuilt/ — no
  nightly anywhere near the user).

- **night-improve-26** — the kernel compatibility record:
  docs/KERNEL_COMPATIBILITY.md. The kernel floor the tap requires
  (5.15, with the honest doctor verdict either way) plus one entry
  per distro layout the tap actually meets, each carrying its
  evidence — opened with the CachyOS nested-bpffs case: the layout
  (`findmnt -t bpf` shows bpf-on-bpf), the symptom (the first-ever
  attach's EEXIST at /sys/fs/bpf/zylaxion), the classified-cleanup
  fix, and the provenance chain from the live 6.18.50 host (the
  EEXIST, the detach clearing the leftover, the clean attach,
  `bpftool prog show` reporting `prog_type tracing` on the BTF id
  of `input_event`). A resolved entry is RESOLVED — not worked
  around: the fix lives in the code, so nobody re-litigates it as
  a zylaxion bug later. Indexed from docs/README.md.

- **night-master-2** — the one-command machine installer,
  `install-on-my-machine.sh` at the repo root. Linux only, no flags:
  it reads the CPU's x86-64 microarchitecture level from /proc/cpuinfo
  (x86-64-v4 when the full AVX-512 set is present, x86-64-v3 for the
  AVX2/FMA/BMI class, baseline otherwise), builds the matching shape
  through the parity-checked cargo aliases (`pro-linux-amd64-v4-gnu`,
  `pro-linux-amd64-v3-gnu`, plain `--release` fallback for baseline or
  non-amd64 hosts), then installs the binary to `/usr/bin/zylaxion`
  and the boot oneshot to `/etc/systemd/system/zylaxion-attach.service`.
  The same commit collapses `scripts/package/install.sh` to the
  system-only posture — before: `--user` (default, `~/.local/bin`, no
  unit) versus `--system` (`/usr/bin` plus the unit); after: one
  posture, `/usr/bin` plus the unit, no flags. Both installers clean a
  stale `~/.local/bin/zylaxion` first so nothing shadows the system
  binary. `scripts/package/uninstall.sh` keeps its user-path sweep as
  legacy cleanup for installs made before this change, and README plus
  docs/RULES.md describe the new posture (the release-binary download
  example now lands `/usr/bin` too).

- **night-improve-25** — the Pro mode section on the one help
  reference (the zelynic NIGHT-improve-45 lineage): every hidden and
  advanced spelling documented COMPLETELY, grouped by the lane that
  owns it — the three short aliases (`-s, --switch <NAME>`,
  `-p, --percent <0-100>`, `-v, --verbose`) with their runnable
  examples (the short spellings are new to the surface; the Run
  section keeps the long ones, the no-duplicate law), the four
  environment spellings of the no-config law (`ZYLAXION_PRESET`,
  `ZYLAXION_VOLUME` with its linear-scale warning — 5.5 laptop
  speakers, 1.5 headphones, a different unit than the percent lane —
  `NO_COLOR=1`, `RUST_LOG` with the log lane's split defaults), and
  the one hidden verb (`zylaxion _play`, the internal stateless
  player, with the foreground-debug teaching its own error strings
  already hand users). One computed description column across every
  group and its continuations (the hunt-35 geometry law, adapted:
  the column derives from the estate's longest spelling and cannot
  drift per row); tier law holds — flags and the verb synopsis grey,
  environment rows default prose (an environment variable is not a
  flag, the hunt-39 boundary), examples green. The drift pin in
  test/commands/help_tests.rs fails the suite if any spelling silently
  thins back to hidden, and its negative arm fails it if the bare
  `play` spelling is ever taught as a verb. README's usage section
  gains the pointer paragraph; the help demo screenshot refresh
  follows at the next owner-host capture session.
- **night-improve-25** — the classified attach failure surface:
  the `AttachError` taxonomy (nine locked variants —
  FentryUnavailable, KprobeBlacklisted, SymbolMissing,
  VerifierRejected, PermissionDenied, BpffsUnavailable, PinFailed,
  LockdownActive, Unknown), pure errno classifiers pinned by the
  test matrix; the failure marker, a one-entry array map planted
  on bpffs by a failed attach carrying the variant's stable code,
  which the doctor reads back to name the last failure after the
  process is gone; the negative help pin against the tracepoint
  fiction's teaching string (the drift gate the Fixed entry
  names); and the probe's four
  canonical input-tap verdict strings. The prebuilt-object
  manifest's tree-hash recipe is documented and automated
  (`scripts/dev/regenerate-ebpf-prebuilt.sh`) — the value
  committed by night-improve-23 was generated by an unrecorded
  command and could not be reproduced from history.
- **night-improve-24** — the kernel-side CI lane: `eBPF tap build`
  proves the detached ebpf/ workspace standalone on its own dated
  nightly — `cargo fmt --check` inside ebpf/, the locked
  `bpfel-unknown-none` release build with `-Z build-std=core` through
  bpf-linker, and ELF structural validation of the object (real ELF
  magic; the tracepoint, maps and license sections aya's loader
  demands). No system packages: aya-ebpf is pure Rust. The check lane
  remains the embedding half's proof; this lane is the kernel half's.

### Removed

- **night-improve-23** — the daemon dies. The entire POSIX daemonization
  stack is deleted: fork/setsid with the init-sync pipe, the PID file
  (with its /proc/exe identity forensics), the Unix-domain-socket IPC
  layer (the JSON protocol, the poll-thread hardening, the umask dance),
  the flock instance lock, and the libinput seat capture (the
  `input` crate with it, plus the `input` group requirement). The two
  libinput-era examples (listen_keys, zylaxion_live) retire with their
  subject. serde and serde_json leave the dependency table — there is
  no protocol left to serialize. Eight of the eleven v10.2.0 audit
  findings (B4, B6, B7, B9, B10, B17, S1, S2, P7) were daemon-induced;
  those bug classes are structurally impossible now.

### Added

- **night-improve-23** — the daemonless eBPF control plane, the world
  first: a keyboard-sound tool whose input tap lives in the kernel, whose
  state store is pinned BPF maps, and whose player is a stateless,
  own-nothing process.
  - `ebpf/` — the pure-Rust BPF crate (aya-ebpf 0.2.1, detached
    workspace, dated nightly pin, repo-local bpfel target spec with
    atomic-cas): one tracepoint program (`zylaxion-tap`) on
    `input:input_event` that gates against pinned maps and publishes
    press/release records to a ring buffer. Autorepeat and mouse
    buttons filter in the kernel (documented deltas).
  - `src/ebpf/` — the userspace loader: `attach` (load, raw-syscall
    link pin, control-plane init, pin everything, chown to the sudo
    user), `detach` (unpin everything — afterwards NOTHING of zylaxion
    exists in the kernel), and the aligned embedding (the zelynic
    NIGHT-hunt-30 AlignedElf lesson).
  - build.rs — three object lanes: source-built (nested nightly
    cross-compile with the prerequisite preflight, RUSTFLAGS poison
    strip, v3 force, structural ELF validation, self-heal),
    registry-prebuilt (ebpf-prebuilt/ + provenance manifest), and
    dormant (`--no-default-features` builds a binary whose eBPF
    surfaces answer with the honest refusal).
  - `src/commands/sound.rs` — the CLI verbs: `on` (write the control
    plane, spawn the player), `off` (clear the gate — the player
    self-exits within 100 ms, zero processes remain), `preset` /
    `volume` (live map writes), `stats` (kernel-side per-key telemetry
    accumulated as BPF_ATOMIC fetch-adds at zero userspace cost),
    `status` (kernel verdict + tap residency + player state).
  - `src/commands/player.rs` — the hidden `_play` internal verb: the
    stateless player that reads every byte of state from the pinned
    maps, registers its pid in the control map (the pidfile lives in
    the kernel now), drains the ring buffer through an epoll bridge,
    and applies preset/volume changes live (the ArcSwap model swap and
    the new atomic volume cell in the voice pool).
  - `src/session/kernel.rs` — the kernel capability probe: release
    parsing, bpffs mount check, BTF presence, tracepoint visibility,
    lockdown mode, and the canonical verdicts against the 5.15 owner
    minimum ("verified supported" / "verified unsupported") wired into
    the doctor and the attach gate.
  - assets/zylaxion-attach.service — the boot ONESHOT (Type=oneshot,
    not a daemon) that plants the tap once per boot; bpffs pins do not
    survive a reboot, the oneshot replants them.
  - scripts/dev/bootstrap-ebpf.sh — the host bootstrap for developing
    the tap (dated nightly + the pinned bpf-linker prebuilt).
  - packaging — install.sh/uninstall.sh teach the oneshot posture and
    sweep the kernel side (best-effort `detach`) on removal.

### Changed

- **night-improve-23** — the CLI surface speaks the daemonless world:
  `start`/`daemon`/`stop` are gone; `on`/`off`/`preset`/`volume`/
  `stats`/`status` are one-map-write verbs that live under 5 ms. The
  doctor reports the kernel/eBPF verdict instead of the input group
  (the group died with the seat capture — the passive tap needs no
  device permissions). The volume retune rides a new atomic cell in
  the voice pool (`set_master_volume` takes `&self` now, wired through
  the orchestrator's `volume_factor` handle). nix trims to
  signal/process/user/poll; libc survives solely for the raw
  BPF/perf syscall lane in src/ebpf/bpf_syscall.rs (aya's typed
  TracePointLink wraps a pub(crate) inner, so its link cannot be
  pinned through the public API — the zelynic bpf_syscall precedent).

### Removed


- **night-improve-22** — the no-config law. `config.toml` (1672 lines)
  and its entire support stack are deleted: the session search-path
  module (src/session/config/), the TOML parse/validate surface, the
  mtime config-watcher thread (a 1-second filesystem poller), the
  `testconf` subcommand, the per-key `[[keys]]` override machinery
  (KeyOverride, the Override* field-patch structs, ProfileWithOverrides,
  MechanicalClick::with_overrides), the MasterParams [master]-table
  carrier, and the `toml` dependency (the lock graph sheds the
  toml/toml_edit/toml_write/serde_spanned branch; serde survives solely
  for the daemon IPC protocol). Owner's verdict: hard to maintenance,
  low gains investment, absurd for keyboard sound.

### Added

- **night-improve-22** — the masterclass replacement, the compiled
  preset registry. All 24 acoustic presets are now compile-time Rust
  data in src/profiles/registry.rs, transcribed byte-for-byte from the
  retired config.toml and pinned in-range by a new test tree (each
  value must survive validate_and_clamp with zero drift — the old
  testconf guarantee, enforced at test time on typed data). Preset
  selection is the Unix configuration surface: `--preset <name>` on
  the CLI > `ZYLAXION_PRESET` in the environment > the compiled
  default (`technical`, unchanged as the known-good fallback). The
  master output volume follows the same law via `ZYLAXION_VOLUME`
  (default 5.5; invalid values are hard errors — the no-silent-
  fallback contract survives). A preset is chosen once at process
  start; switching is a daemon restart — the honest lifecycle for a
  sound switcher. `list-presets` renders the catalog with one-line
  descriptions; unknown names keep the GNU-shaped error with the
  did-you-mean tip; the systemd unit gains a commented
  `Environment=` preset pin. install.sh ships two artifacts instead
  of three; uninstall.sh cleans legacy config-era directories.
  Net tree delta: about 900 lines deleted, 6 dependencies shed.
  Suite: 154 tests green (five consecutive runs), clippy
  --all-targets -D warnings clean, version untouched at 11.0.0.

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
