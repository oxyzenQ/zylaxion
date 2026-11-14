<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: AGPL-3.0-only -->

# The zylaxion Innovation Ledger

This page records the research and innovations that define zylaxion's
sound engine and its kernel tap. It is the result of the owner's own
thinking and design research, conducted together with AI in 2026 —
written simple on purpose, so a new maintainer (or a curious
competitor) can learn the whole instrument in one sitting. Each entry
names the problem it kills, the mechanism in one breath, and the file
where the truth lives. The version contract itself is owner-decided
(root `Cargo.toml`, `ebpf/Cargo.toml`, and `ebpf-prebuilt/manifest.toml`
must agree — the version mirror law); this page is the map, not the
changelog. One law runs through everything below: **no samples, no
daemon, no config file** — every keystroke is computed, the kernel
owns the state, and the only resident thing between keystrokes is a
passive eBPF program. The pure cores (`src/engine/`, `src/profiles/`,
`src/core/`) are pinned by in-tree unit tests rather than hope, and
the tuning surface is judged by a headless audit harness, not by ear.

## The engine, named

The core these entries compose has a name: **Zactrix** — the
time-variant synthesis engine behind every keystroke, and the name of
the whole procedural model (engine, profiles, presets, audit
instrument). The unified package is **the cosmic dragon engine and
architecture** (night-improve-21): the former six-crate workspace
(zactrix-engine, zactrix-profiles, zylaxion-core, zylaxion-input,
zylaxion-output, zylaxion) collapsed into ONE package at the repo
root, one target, one lock graph, one build — the former crate
boundaries survive as the unified module tree. Inside the module tree
the runtime metaphor is the body: the eBPF tap is **the Ears**, the
`Orchestrator` is **the Brain**, the voice pool is **the Voice**, and
the cpal sink is **the Mouth** (`src/core/mod.rs`). The naming keeps
the family's one law — one identity, one name, many engines: zelynic
enforces bandwidth, zylaxion voices keystrokes, both speak the same
"law" idiom and ship the same honesty rule: a claim must be testable
or it does not ship.

The honest edge is stated on purpose: zylaxion is a synthesizer, not
a sampler. Every sound is computed from shaped noise and TPT state
variable filters, so the measure of truth is the audit harness —
spectral centroid drift, flatness evolution, modal decay spread over
the 0–100 ms onset window at 96 kHz (`scripts/audit/`, documented in
`docs/ZACTRIX_AUDIT.md`) — and the preset registry refuses to pretend
that every invented profile survived verification: four identities
(whisper, rotary_encoder, cyber_deck, glass_tactile) stay as authored
because the owner accepted them, not because the harness blessed them.

## The heart: the Zactrix synthesis core

**1. The Zactrix time-variant spectral envelope** (stage A/B/C).
The frozen-coefficient hole: a static filter bank sounds like a
beep dressed as a keypress — `docs/ZACTRIX_AUDIT.md` measured the old
model's presets carrying 99+ percent of onset energy in the first
5 ms. The fix bakes physics-shaped cutoff and resonance *curves* into
each voice at trigger time, then recomputes the filter coefficients
every sample while a curve is still moving: stage A (click) starts
broadband (2.5x the click band, clamped below 0.45 Nyquist) and
collapses into the click band with a 1 ms time constant; stage B
(spring) travels from the click band to the spring band over roughly
5 ms while resonance wakes; stage C (housing) holds constants.
night-depth-hardening-1 adds the frozen-coefficient lanes: once a
curve's remaining travel falls under a quarter rounding bucket, both
stages' coefficients are provably final and the per-sample tan()
leaves the loop — the voice tail runs pure IEEE-754 mul/add, byte-
identical to the unfrozen render (the rates audit proves it) and
free of libm variance across distros and glibc versions. Files of
truth: `src/profiles/mechanical/init.rs` (the shape constants, e.g.
`TAU_CLICK_S = 0.001`) and `src/profiles/mechanical/render.rs`
(the per-sample coefficient path, cutoff clamped to `[20, 0.45*sr]`
Hz and Q to `[0.1, 30.0]`). night-depth-mariana-1 completes the
byte-identity thread with the denormal policy law: `VoicePool`
construction pins the render lane's floating-point control word —
FTZ on x86-64, FZ on AArch64, the same output-flush contract in
each control word (DAZ was measured byte-irrelevant — FTZ+DAZ and
FTZ-only render identical hashes — and is deliberately excluded) —
one write per pool, never per sample. The voice tail's subnormal band (the
audit's dust finding) stops paying SSE microcode assists: the active
lane measured 71 → 38 CPU ticks per ~55 s window (~1.29% → ~0.69% of
one core), the byte delta against the pre-law ambient posture is
proven confined 253 dB below the -60 dB measurand floor (the
`--denormal-delta` instrument), and the contract no longer depends
on whatever control word a host or library left behind. File of
truth: `src/engine/denormals.rs`; pinned by
`test/engine/denormal_policy_tests.rs`; the trade and the
re-baseline are recorded in `docs/ZACTRIX_AUDIT.md`. night-depth-
mariana-2 pins the cross-arch facts with the parity lane
(`scripts/audit/zactrix-rates-parity.sh`, the committed same-arch
baselines, `.github/workflows/rates-parity-arm64.yml`): the FZ pin
is read back from FPCR on the arm64 runner and the law's delta is
dust-confined there too (-313.1 dB, 0 gate failures — the same
split as x86-64); the lane also MEASURED where cross-arch byte
identity stops — the attack fingerprints (init-shaped
coefficients) differ on every preset, the init-time transcendentals
resolve through per-target libm — a boundary that is recorded in
every run log and closed later by the correctly-rounded pure-Rust
transcendental lane.

**2. The triple TPT SVF chain.** The thinness problem: a single
filter leaves the high-frequency click present but the body missing.
Three parallel Topology-Preserving-Transform state variable filters
(trapezoidal integrator, `a1 = 1/(1+g(g+k))`) — click, spring,
housing — are driven by shaped white-noise bursts; the click lane
mixes its highpass and bandpass outputs (0.7/0.3), and the housing
bandpass is multiplied by its own Q so `housing.mix` acts as a true
level control instead of a second resonance knob. File of truth:
`src/profiles/mechanical/render.rs`.

**3. The housing drive-window law.** A HIGH-Q housing ringing free
would be slurped by a long driven window, and a low-Q housing mode
needs ring-up time it does not have. The law picks the excitation
window per keystroke: when housing Q exceeds 3, the window equals the
click excitation window (impact contact, rings free); otherwise it
spans at least four periods of the housing frequency. File of truth:
`src/profiles/mechanical/init.rs`.

**4. The sample-rate-normalized decay law** (`c_S = c_R^(R/S)`).
The device-rate hole: applying a decay coefficient raw made the same
preset ring a different length per sound device — the audited example
was a 1237 ms tail at 44.1 kHz collapsing to 568 ms at 96 kHz. Every
per-sample decay coefficient is re-derived per device rate as
`coefficient.powf(SAMPLE_RATE / device_rate)` before use, applied to
the main decay, the fast stage, and the ambient envelope. File of
truth: `src/profiles/mechanical/init.rs` (which also records the
before/after numbers in its comments).

**5. Micro-randomization (splitmix64 + xorshift32).** The uncanny
valley: a deterministic synthesizer is perceptibly synthetic —
identical keystrokes, identical sound. Each keystroke counter is
hashed through splitmix64 into a per-keystroke seed; seven xorshift32
rolls produce pitch drift (+/- 1.5 percent), amplitude drift
(+/- 5 percent), decoupled noise seeds, and pan jitter (+/- 3
percent). All randomness is resolved at trigger time, never in the
audio callback, so the render path stays branch-light and allocation
free. Files of truth: `src/profiles/mechanical/mod.rs` (the seed
mixing, including the autorepeat metallic-ringing fix) and
`src/profiles/mechanical/init.rs`.

**6. Decoupled noise streams.** The honk problem: click bandpass and
housing bandpass consuming one identical noise stream merged into a
single layer through constructive interference. Three independent
xorshift32 streams are seeded per keystroke with distinct magic
constants (`KEYB`, `HOUS`, `AMBI`), and the housing fade uses
exponential decay where the click uses a quadratic window, so the
layers stay physically distinct. Files of truth:
`src/profiles/mechanical/render.rs` and `src/profiles/mechanical/init.rs`.

**7. The equal-power pan law with a scancode pan table.** Stereo
placement from physics: gain pairs follow the equal-power law
(`cos`/`sin` of the normalized angle), and the evdev scancode is
mapped through keyboard-row heuristics (nav cluster and numpad wide,
space center) so a real keyboard pans like a real keyboard. Files of
truth: `src/core/pan.rs` and `src/profiles/mechanical/init.rs`.

**8. Inter-keystroke interval attenuation.** The machine-gun tell:
real typists produce softer second keystrokes in a fast double. The
tap's monotonic trigger timestamps feed a law that attenuates
amplitude by up to 5 percent when the previous keystroke landed within
80 ms. Files of truth: `src/profiles/mechanical/init.rs`,
`src/profiles/mechanical/render.rs`, and `src/core/mod.rs`.

**9. The soft release ramp with a two-stage decay.** The click-off
tell: a hard 1-sample envelope cut is itself audible as a tiny click.
Release never hard-cuts — the voice ramps out over roughly 2 ms to
the 1 percent line — and profiles can carry a fast initial decay
stage before the main exponential tail. File of truth:
`src/engine/pool/mod.rs` (release ramp) and
`src/profiles/mechanical/render.rs` (two-stage decay).

**10. Analog velocity (Hall-effect) scaling.** An optional velocity
input (`None` = digital full scale) scales amplitude linearly and
resonance toward brightness for harder presses (up to 20 percent),
so analog keyboards can shape each keystroke's timbre. Files of
truth: `src/profiles/params/mod.rs` and
`src/profiles/mechanical/init.rs`.

**11. The VoicePool: pre-allocated voices, oldest-first stealing.**
The real-time hole: allocation in an audio callback is a dropout.
The pool is a fixed array of 16 voices
(`MAX_POLYPHONY = 16`) initialized once, voices are stolen by the
smallest monotonic trigger timestamp, and the render path allocates
nothing. Files of truth: `src/engine/pool/mod.rs` and
`src/engine/voice/mod.rs`; stealing and smooth release are pinned by
tests in `test/engine/pool_tests.rs`.

**12. The hard clamp and the NaN silence net.** The invariants: a
channel never leaves the engine outside `[-1.0, 1.0]`, and a
non-finite sample becomes silence, never a crash — "the worst the
user hears is a brief moment of silence". Defense in depth: the pool
clamps per channel and the cpal callback clamps again. Files of
truth: `src/engine/pool/mod.rs` and `src/audio/mod.rs`.

**13. The atomic volume cell with a single clamp law.** The ranged
hole: a live retune path once stored raw bits, so a forced retune
could outrange what a fresh session would clamp. Master volume lives
in one atomic f32-bits cell (`Arc<AtomicU32>`), and every writer —
startup or live — goes through the same clamp (finite, within range,
else the 5.5 default). One cell, one law, no divergent writers. File
of truth: `src/engine/pool/mod.rs`.

**14. The compiled preset registry with the validate_and_clamp
contract.** The config-parse hole it refuses: no config file means no
parse errors are possible — 24 presets ship as a static registry
packed by `const fn` constructors (`PRESETS: [Preset; 24]`),
type-checked at compile time and range-checked by the test tree. Any
externally supplied profile must survive `validate_and_clamp`, which
enforces the documented ranges (click 500–8000 Hz, resonance
0.1–30.0, decay coefficient strictly below 0.9999, housing
100–1000 Hz) — and the tests pin that every preset survives clamping
without drift. Files of truth: `src/profiles/registry.rs` (the
registry, default preset "technical") and `src/profiles/profile/mod.rs`
(the clamp contract); pinned by `test/profiles/registry_tests.rs`.

**15. The SPSC ring lane (the Mouth bridge).** The realtime hole: the
synthesizer thread and the audio callback must never share a lock.
The engine writes stereo frames into a 16384-frame single-producer/
single-consumer ring (about 370 ms at 44.1 kHz), sized on purpose to
absorb Linux non-RT scheduler jitter spikes of 30–50 ms without the
callback ever seeing an empty buffer — worst-case extra latency is
inaudible for a sound effect that plays for hundreds of milliseconds.
The ring's vacancy side feeds the orchestrator's adaptive wait and
the shutdown silence pad, so pacing decisions always read the real
consumer, never a guess. File of truth: `src/audio/mod.rs`
(`RING_BUFFER_FRAMES`); the bridge is exercised by
`test/core/core_tests.rs`.

## The ears: the passive kernel tap

**16. The passive eBPF tap — one body, two attach programs.** The
everything-hole it replaces: no `/dev/input`, no evdev grab, no udev
rules, no `input` group, no daemon. A pure-Rust aya-ebpf program
probes the kernel's own `input_event()` — through an fentry trampoline
or a kprobe, whichever the chain wins — and gates in the kernel:
when `enabled != 1` the whole cost is one branch; unmapped keycodes
die in the kernel; autorepeat never sounds. Events ride a 64 KiB
`RingBuf` that silently drops when full ("a missed click sound is
always preferable to blocking the input path"), and stats counters
use 64-bit atomic fetch-adds. Files of truth: `ebpf/src/main.rs`
(maps: control, keymap, events, stats) and the ABI tests in
`test/ebpf/args_tests.rs`.

**17. The attach chain (fentry, then kprobe, then kprobe-alt) with
the TAP_KIND slot.** The portability problem: one attach method does
not survive every kernel. The chain is a locked three-step ladder —
fentry on `input_event` (kernel 5.5+), kprobe on `input_event`, then
kprobe on the alternate symbol — and the winner is recorded in the
control map's `TAP_KIND` slot so status and doctor read the same
vocabulary the loader wrote. The loaded program is attached at most
once; the symbol is an attach-time parameter. Files of truth:
`src/ebpf/loader/mod.rs` and `src/ebpf/loader/chain.rs`; vocabulary
round-trips are pinned in `test/ebpf/loader_tests.rs`.

**18. The raw-syscall pin lanes.** The library limit: Aya's typed
attach methods return links whose inner handles are not public, and a
link that is not pinned dies with the attaching process — the tap
would vanish the moment `attach` exits. Hand-built `bpf()` syscall
lanes mirror the kernel ABI shapes (raw tracepoint open for fentry;
perf event PMU plus link create for kprobe; each ending in
`BPF_OBJ_PIN`), with a failure-marker lane for diagnostics. Files of
truth: `src/ebpf/bpf_syscall.rs`; the ABI shapes are pinned by
`test/ebpf/bpf_syscall_tests.rs`.

**19. AlignedElf — alignment-guaranteed embedding.** The rustc trap:
`include_bytes!` produces an align-1 static, and the eBPF loader
rejects unaligned ELF with a healthy artifact — an error that cost a
real debugging session. The object is embedded through a
`#[repr(C, align(8))]` wrapper carrying a magic guard that also
defeats rustc's const-interner deduplication (verified against
modern rustc), plus an alignment preflight tripwire that fails the
load before the kernel ever sees a misaligned pointer. File of
truth: `src/ebpf/embedded.rs`; pinned by `test/ebpf/embedded_tests.rs`.

**20. The attach failure taxonomy with an on-kernel failure marker.**
The silent-failure hole: an attach that fails after the process exits
leaves nothing to read. `AttachError` is a closed nine-variant
taxonomy where each variant renders a complete sentence and carries a
stable numeric code; a failed attach plants a one-entry pinned array
map under `/sys/fs/bpf/zylaxion/attach_error` that the doctor reads
back after the process is gone. Privilege failures (EPERM, EACCES)
are their own variants, not generic errors. Files of truth:
`src/ebpf/attach_error.rs` and `src/ebpf/loader/orchestrate.rs`;
pinned by `test/ebpf/attach_error_tests.rs`.

**21. Idempotent attach with self-heal and classified cleanup.** The
crash-recovery problem: an interrupted attach can leave a partial pin
set behind. A complete pin set means success (still performing the
chown hand-over); a partial set is cleaned and the stale directory
removed — but removal is classified: entries the tap does not own are
never deleted, and link pins are removed first so an interrupted
cleanup leaves a DETACHED tap rather than a half-attached one. Files
of truth: `src/ebpf/loader/orchestrate.rs` and `src/ebpf/pin.rs`;
removal order is pinned by `test/ebpf/pin_tests.rs`.

**22. The pin hand-over and the bpffs front door.** The ownership
trap: owning a pin is not reaching a pin — a bpffs mount in mode 0700
(the Debian 13 default) blocks every `stat()` through `/sys/fs/bpf`,
and the desktop player runs as the user, not root. Attach chowns the
pins to the invoking desktop user (`$SUDO_USER` or the lowest
`/run/user` uid at or above 1000) and, if the owner cannot walk the
mount, adds exactly `o+x` to `/sys/fs/bpf` — never `o+r`, pins stay
0600. Files of truth: `src/ebpf/loader/chain.rs` (hand-over) and
`src/session/kernel.rs` (the reachability math).

**23. The kernel capability probe — one probe, two consumers.** The
drift problem: attach prechecks and doctor reports that re-implement
their own verdicts disagree. One `probe()` builds the kernel report —
release parse against the 5.15 floor, BTF full-token search for the
attach symbol, bpffs mount check, lockdown mode, and the
`unprivileged_bpf_disabled` sysctl shape with its boot-parameter
cross-check — and both the doctor (print) and attach (hard gate)
consume the same verdicts. The BTF match is documented as a preflight,
not proof. File of truth: `src/session/kernel.rs`; seventeen tests in
`test/session/kernel_tests.rs`.

## The body: lifecycle, privilege, and the stateless player

**24. The stateless player — kernel-owned state, "cattle, not pet".**
The daemon hole it refuses: zylaxion's only resident process is a
stateless `zylaxion _play` whose pid and lifecycle state live in the
pinned control map — "the pidfile lives in the kernel now". The
player polls the pinned ring buffer and re-reads the control map
every 100 ms (preset swap via ArcSwap, volume via the atomic cell),
survives a 10 s read-failure budget before declaring the tap lost,
refuses to coredump, and dies on `off` leaving nothing resident.
File of truth: `src/commands/player.rs`.

**25. The orphan lifecycle: identity gate and escalation ladder.**
The double-sound problem: a player that outlives the map it watched
leaves two players sounding. Before any privileged signal, the
candidate pid is identity-gated by an exact two-token cmdline match —
"the pid it carries is untrusted input to a privileged kill" — and
the reaping decision is a pure decision-table ladder (observe, TERM,
KILL, conclude gone or survived) with bounded 100 ms stages, run
before unpinning. Files of truth: `src/session/orphan.rs` and
`src/ebpf/teardown.rs`; pinned by `test/session/orphan_tests.rs`.

**26. The privilege audit table.** The sudo-everywhere disease it
kills: who may run what is a doc-table policy — `on` and `_play`
REFUSE under root (they belong to the desktop user), `attach`
REQUIRES root, `off` and `status` stay allowed as rescue verbs, and
the refusal wordings are pure functions of the euid fact so tests pin
the contract exactly. File of truth: `src/session/privilege.rs`;
pinned by `test/session/privilege_tests.rs`.

**27. The no-config law: preset resolution and volume caps.** The
config-file hole it refuses: resolution is a fixed order — CLI flag,
then environment, then the control map's stored slot, then the
compiled default ("technical") — and the environment lane is capped
so `on` cannot smuggle a louder session in through the env back door:
absolute gain at or below 10.0, percent lane capped at 200 percent,
10000 percent only behind the explicit `--force-this`. Refusals are
hard errors, never silent fallbacks. Files of truth:
`src/session/preset.rs` and `src/commands/sound/tuning.rs`;
twenty-five tests in `test/session/preset_tests.rs`.

**28. The errno law for "not attached" verdicts.** The folded-error
hole: `Path::exists` folds EACCES into `false`, so a mode-0700 bpffs
mount makes an attached tap read as absent and `off` would report
nothing to do. Pin presence is classified from `stat()`: only
ENOENT means absent, only EACCES means walk-blocked, and every other
errno defers to the classified open — an unseen errno never folds
into an idempotent verdict. File of truth:
`src/commands/sound/mod.rs`; the classification is pinned by
`test/commands/sound_tests.rs`.

**29. The orchestrator adaptive wait.** The idle-CPU hole: a fixed
1 ms polling loop burned a thousand futex wakeups per second — the
audited cost was roughly 1.9 percent CPU, 76 CPU-minutes on the
owner's machine. The wait adapts: 1 ms while the pool is active,
100 ms while silent, an instant wake on event arrival, and a 500 us
sleep when ring vacancy runs low, with a heartbeat log ladder
(60 s debug, 5 min warning). File of truth: `src/core/mod.rs`.

**30. The shutdown silence pad.** The pop problem: dropping the
audio sink mid-playback can leave a non-zero offset in the sound
server's buffer, heard as a pop; and a disconnected device makes
vacancy read zero forever. Shutdown writes a 1024-frame silence pad
paced against the ring's producer vacancy, bounded by a 300 ms
deadline so a dead device cannot stall exit. File of truth:
`src/core/fade.rs`.

**31. The engine freeze.** The stability contract: the engine is
frozen at a named commit — changes require a documented why that
passes the maintenance bar — and the freeze itself is enforced by
doc-pinning tests so a doc edit cannot silently unpin it. Files of
truth: `src/engine/mod.rs` and `test/engine/freeze_tests.rs`.

## The workshop: build, audit, and CLI tooling

**32. Two-lane eBPF staging with blake3 pinning.** The reproducibility
problem: the tap object must be byte-traceable to its source. A git
checkout cross-builds the nested eBPF workspace with a dated nightly
pin and a committed target spec; otherwise the registry prebuilt
object is staged only after structural ELF validation and blake3
verification against `ebpf-prebuilt/manifest.toml` (schema 2) — a
stale or tampered lane panics naming both hashes and never silently
embeds an object the manifest does not describe. The manifest also
pins the exact `ebpf/` source-tree hash and mirrors the version
contract. Files of truth: `build/ebpf.rs`, `build/prebuilt.rs`,
`build/blake3.rs` (a dependency-free BLAKE3 with test vectors), and
`ebpf-prebuilt/manifest.toml`.

**33. The commit-sha chain and the no-time-crate build stamp.** The
supply-chain rule: the build hash resolves through a fixed chain
(git rev-parse, then `GITHUB_SHA`, then the vcs-info file), and the
build date is computed by the Hinnant civil-calendar algorithm —
"chrono, jiff, and friends stay out of the dependency tree entirely";
the same algorithm runs in the runtime clock lane so report stamps
match build stamps. Files of truth: `build.rs`, `src/info/clock.rs`,
and `src/info/build.rs`.

**34. The Zactrix audit instrument — verifier-first tuning.** The
tuning-by-ear trap it replaces: the headless harness renders all 24
presets through the real render path at 96 kHz and measures physical
metrics over the onset window (spectral centroid drift, spectral
flatness evolution, modal decay spread); `zactrix-verify.sh` judges,
and range expansions (Q to 30.0) are backed by a 1M-sample stability
battery with zero NaN, zero Inf, and a retention-law spread within
0.02 percent. Files of truth: `scripts/audit/` and
`docs/ZACTRIX_AUDIT.md`.

**35. The CLI rescue machinery and the one-palette output law.** The
typo wall it removes: preset typos rescue through Levenshtein
matching (distance 2, case-insensitive, deterministic ties) and long
flags through Jaro similarity against the failing subcommand's pool;
every fatal path ends with exactly one canonical help footer, and
clap and the output layer share one five-color palette so the brand
cannot drift per-verb. Files of truth: `src/cli/suggestion.rs`,
`src/cli/ux.rs`, `src/output/mod.rs`, and `src/cli/styles.rs`;
pinned by `test/cli/ux_tests.rs` and friends.

**36. The journal one-door and the quiet-ALSA guard.** The log-spew
problem: device probes on a cardless host can print raw ALSA C-level
errors across rendered CLI surfaces. All verbs log through one
journal door (UTC stamps from the Hinnant clock), and an RAII guard
swaps libasound's error handler for a no-op during probes and
restores it on drop. Files of truth: `src/output/journal.rs` and
`src/commands/info/alsa_guard.rs`.

**37. The zero-TLS update check.** The supply-chain surface it
refuses: `--check-update` shells to `curl` against the GitHub
releases API with a 15 s timeout and parses the tag with a std-only
string parser and SemVer comparison — no JSON crate, no HTTP crate,
no TLS stack in the dependency tree; the root-user refusal exits
before any network I/O. Files of truth: `src/commands/update.rs` and
`src/info/version.rs` (fuzz-target covered).

**38. The provenance depth matrix — the masked ownership seal.** The
rebrand threat it kills: a thief greps the artifact with `strings(1)`
or `rg`, finds the plaintext identity, and patches it out. The claim
"rezky_nightky (oxyzenQ) 2026" ships only as splitmix64-XOR-masked
little-endian u64 words — two independent layers, the Signature
manifest and the Copyright authors — with FNV-1a 64-bit fingerprints
const-evaluated from the masked words at compile time and recomputed
at runtime through a `black_box` barrier LLVM cannot fold, a
cross-layer identity binding (the manifest's identity core must
prefix the authors string), and a blake3 anchor over the framed
payload (domain tag + length-prefixed layers) generated by
`scripts/release/generate-provenance-words.py` — the committed truth
source, carrying its own spec-exact pure-Python BLAKE3 pinned to the
official vectors. `provenance_view()` materializes the plaintext in
ONE unmask pass per layer (the single-pass invariant: the displayed
bytes are exactly the bytes the integrity folds); `-V` reports
`status=verified` or `tampered` on the Depth-matrix line, and
`zylaxion sig` walks the four verification depths (runtime identity,
layer binding, anchor integrity, the running image's own self-hash
fingerprint — a fingerprint by law, it cannot embed its own digest)
and exits 2 on any tamper. Files of truth: `src/info/signature.rs`,
`src/commands/sig.rs`, and
`scripts/release/generate-provenance-words.py`; pinned by
`test/info/signature_tests.rs` and `test/commands/sig_tests.rs`.

## The test tree that pins the ledger

The ledger is not honored by prose alone: 28 test files carrying 234
test functions pin the entries above, wired into the crate through
`#[path]` modules. The family map, so an auditor can walk it in one
pass:

- `test/profiles/` (mechanical, profiles, registry) — the Zactrix
  stages, the TPT chain, the housing window, the rate-normalized
  decay law, micro-randomization, the decoupled noise streams, the
  pan table, interval attenuation, velocity scaling, and the registry
  clamp contract (entries 1–10, 14).
- `test/engine/` (pool, render, freeze) — oldest-first stealing, the
  smooth release ramp, the clamp and NaN silence net, and the engine
  freeze declaration (entries 9, 11, 12, 31).
- `test/core/` — the Orchestrator: the pan law, the timestamp lane,
  the adaptive wait, and the ring bridge behavior (entries 7, 8, 15,
  29).
- `test/ebpf/` (args, loader, bpf_syscall, embedded, attach_error,
  pin) — the tap ABI shapes, the chain vocabulary and its control
  slot, the raw-syscall lanes, the AlignedElf guard, the failure
  taxonomy's wire format, and the classified pin cleanup (entries
  16–21).
- `test/session/` (kernel, orphan, privilege, preset) — the capability
  probe, the orphan identity gate and ladder, the privilege table,
  and the no-config resolution and caps (entries 23, 25–27).
- `test/commands/` (player, sound, sig, info, help) — the stateless
  player vocabulary, the errno classification law, the sig report
  shape, and the doctor surfaces (entries 24, 28, 38).
- `test/cli/`, `test/info/`, `test/output/` — the CLI rescue
  machinery, the build-stamp chain, the version grammar (fuzz-target
  covered), the journal door, and the provenance layer pins (entries
  33, 35–38).

A test that names a law outranks any doc that describes it: when the
two disagree, the test is the evidence, the doc gets the PR.

## How to verify every claim above

The honest rule of the family: source code is the single source of
truth, and every entry above names its file. The fast paths:
`cargo test` runs the in-tree unit test tree that pins the bounds,
laws, and round-trips named above; `scripts/audit/zactrix-audit.sh`
and `zactrix-verify.sh` re-run the physical metrics on the real
render path; `zylaxion doctor` prints the kernel report the attach
gate consumes; `zylaxion status` reads the TAP_KIND slot the loader
wrote. Where a number in any other doc disagrees with the code, the
code wins — and the doc gets the PR.

<!-- ZYLAXION-DISCLAIMER -->
<!--
  Documentation Disclaimer — read before relying on any data point.

  This document may contain stale data, hardcoded counts, or outdated
  file paths and symbol names. Maintainers update source code but may
  forget to sync every doc — perfect sync across every .md file is a
  known maintenance burden with diminishing returns.

  Source code (`src/**/*.rs`) is the single source of
  truth. Always cross-check against the actual source files before
  relying on any specific number (preset count, rate bound, LOC),
  file path, function name, or config key.

  If you find a discrepancy, please open a PR — the doc is wrong, not
  the source.
-->
