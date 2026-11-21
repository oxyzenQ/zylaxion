<!--
  SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
  SPDX-License-Identifier: AGPL-3.0-only OR Commercial
-->
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
transcendental lane. night-frontier-3C removes the self-inflicted
half of that wobble NOW: the f64 curve-anchor law — every curve
constant init_state bakes (the envelope steps, the curve deltas, the
housing TPT trio, the pan pair, the high-pass coefficient, the
decay/release/ambient powers) is computed in f64 and quantized ONCE
to f32 through one `anchor` rounding, and init_state runs off the
audio callback, so the wider precision costs nothing at runtime.
Measured on the A/B machine over the shipped domain: the anchor now
sits within half an f32 spacing of the f64 truth on EVERY family
(the correctly-rounded guarantee, 266-test suite), where the pre-law
f32 chains lost up to ~47 spacings on the high-pass coefficient at
96 kHz, ~72 at 192 kHz, and — where a rounded intermediate feeds a
transcendental whose result lands near zero — up to thousands of
spacings on the pan pair at the stereo extremes (6413 at
p = 1 - 1/8192; the f32 theta rounds onto pi/2 itself at the exact
extreme). The honest limit, recorded with the same numbers: the
pure-exp step and pure-pow families measured at or below one spacing
pre-law on this domain under the host glibc (their intermediates
were already correctly rounded) — the law gives them one consistent
rule there, not accuracy; Miri's own libm then measured the pre-law
f32 powf at one-plus spacing on the same expression, so the pre-law
upper bound is a libm property while the new-anchor guarantee held
under both libms (the Miri failure is recorded, not hidden);
and the fit is untouched: `zactrix-verify.sh` A/B against the
pre-law build reports identical metric rows and grades on every
preset (the C/B grades some presets carry are pre-existing and
unchanged). Cross-libm byte-identity was the deferred transcendental
lane's job — night-frontier-Z5 (entry 47) landed it: the libm
answers left the audio path and that boundary is closed. Files of
truth: `src/profiles/mechanical/init.rs` (`anchor`, the f64 shape
constants); pinned by the `night_frontier_3c_*` tests in
`test/profiles/mechanical_tests.rs`.

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

**43. The mode-state fold, measured and refused (night-frontier-Z3
C1).** The proposal: when the pool saturates, fold the new excitation
into the oldest sounding resonator (the TPT integrator state keeps
accumulating drive, phase continuity preserved) instead of stealing
it — polyphony's ceiling becoming "16 mode-states with drive
accumulation" instead of 16 discrete voices, the mid-ring tail never
cut. The freeze bar's own price named the referee first: a measurable
improvement proven by the audit harness, or the honest skip. The
harness grew the saturation-boundary instrument (`--fold-proof`): a
fixed 20-press burst at 10 ms spacing — a 100 Hz repeat, already
faster than any keyboard, autorepeat, or macro — through the REAL
pool, reporting the boundary voice's own remaining tail energy
against its own onset. The measurement refused the premise: at the
boundary the oldest voice carries −403.5 dB (technical) and
−719.3 dB (glass_tactile) of its onset energy — the click and spring
rings die inside the first tens of milliseconds and the housing band
goes with them; the forced-ceiling shape (every stage's resonance at
the legal maximum, the slowest ring the parameter space allows, the
stability battery's own posture) still carries only −108.7 dB, some
48 dB under the −60 dB measurand floor the delta instrument already
gates at. The one regime where a cut carries audible energy — 2 ms
spacing (a 500 Hz repeat) at ceiling resonance, −18.6 dB there — is
a regime no keyboard reaches: the pool fills in 15×spacing, hardware
repeat tops out near 30 Hz, and the fill alone pushes the oldest
voice far past every preset's own −60 dB tail endpoint. The decision
the owner's law produces: the steal stays, the fold never lands, and
the refusal is checkable forever — one command re-runs the rows, so
any future preset whose ring survives into the reachable regimes
faces the same numbers again. Files of truth:
`scripts/audit/harness/src/foldproof.rs` (the mode) and
`scripts/audit/fold-proof-steal.txt` (the committed run the
entry quotes).

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
use 64-bit atomic fetch-adds. The ring record also carries a
four-byte sound-class `slot`, copied from the same keymap lookup
that gates unmapped keys — zero added per-event cost, and a
deliberately reserved lane: the engine ignores it today (every key
in a preset sounds the same class), and wiring it (per-class
parameters from keycap mass and travel physics) is an owner
decision, because it would re-baseline every preset's rendered rows
through the audit harness. Files of truth: `ebpf/src/main.rs`
(maps: control, keymap, events, stats), the userspace mirror's
field record (`src/ebpf/loader/mod.rs`), and the ABI tests in
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
`unprivileged_bpf_disabled` sysctl shape with its gate-era law
(night-ebpf-era-1: 5.15-5.18 gate every plain-user bpf() call at the
dispatch level, 5.19+ narrow the gate to object creation so the
pinned-fd ops ride the pins' file permissions, and the transition
law is the mainline handler's own — the permanent lock lives at 1)
— and both the doctor (print) and attach (hard gate)
consume the same verdicts. The BTF match is documented as a preflight,
not proof. File of truth: `src/session/kernel.rs`; nineteen tests in
`test/session/kernel_tests.rs`.

## The body: lifecycle, privilege, and the stateless player

**24. The stateless player — kernel-owned state, "cattle, not pet".**
The daemon hole it refuses: zylaxion's only resident process is a
stateless `zylaxion _play` whose pid and lifecycle state live in the
pinned control map — "the pidfile lives in the kernel now". The
player polls the pinned ring buffer and re-reads the control map
every 100 ms (preset swap via ArcSwap, volume via the atomic cell),
survives a 10 s read-failure patience before declaring the tap lost,
refuses to coredump, and dies on `off` leaving nothing resident.
night-hardening-2 evented the control plane (a zero-payload SIGUSR1
after every map write), and night-frontier-Z2 B1 completed it: every
verb drives the pinned notice relay with ONE `BPF_PROG_TEST_RUN`
after its map write — the KERNEL produces the record into the pinned
notices ring (the only publish a ringbuf admits: producer pages are
read-only to userspace on every kernel), the watcher's `poll(2)`
spans the notice ring and the wake pipe (the signal stays as the
fallback the relay's wake crosses the identity boundary with), and
the map re-read that follows is the only authority. The 100 ms poll
demoted to a 10 s liveness heartbeat — the night-dragon-lts read-
failure patience survived, honestly re-derived from a read count to
a clock — and `off`, `preset`, and `volume` land in microseconds on
every kernel from the 5.15 floor up. night-frontier-Z4 demoted the
heartbeat again: the watcher's poll is now a FOREVER block (zero
timer wakeups while the lanes live) and the 10 s heartbeat survives
only as the 60 s backstop that arms when the poll set is EMPTY — a
timer is the only remaining wake channel when both evented lanes are
dead, and `off` must always land (entry 46 carries the full law).
Files of truth: `src/commands/player.rs` (the two-fd watcher),
`src/session/wake.rs` (the signal lane), `src/ebpf/notice.rs` (the
schema), and `src/ebpf/bpf_syscall.rs` (the publish lane).

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
(60 s debug, 5 min warning). night-frontier-Z4 completed the arc
(entry 46): the silent half retired into a PARK — a blocking select
over the event channel and the new stop lane with no timeout, zero
timer wakeups while idle (the 1 ms pacing arms only while voices
render) — and the heartbeat log ladder retired with the timer it
rode (a parked loop never wakes to log; the liveness fork stays with
`doctor` and `status`). The stop became a message (`ControlStop`),
because a flag cannot wake a parked thread. File of truth:
`src/core/mod.rs`.

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

**42. The evented control plane, completed — the notice relay and
the pinned ring (night-frontier-Z2 B1).** The gap B1 closed: the
night-hardening-2 wake was a SIGNAL — it needed the player's pid,
process-targeting permission across the identity boundary, and left
no trace an audit could read back. B1 makes the primary wake
kernel-produced evidence: the loader pins TWO more objects (the
`notices` ringbuf and the UNATTACHED `relay` socket-filter program)
beside the rest of the inventory — same cleanup classification, same
hand-over — while the tap body and the engine stay frozen. After a
verb writes the control map it writes kind and value into the
control map's two request slots (the RELAY's argument channel,
never state) and drives the relay with one TEST_RUN; the kernel
sanity-gates the kind, stamps `bpf_ktime_get_ns`, and publishes the
16-byte record the watcher's `poll(2)` wakes on. The transport rides
the map because the first CI leg's verifier refused the
packet-reading shape with its reason beyond the captured log tail,
while the slot shape uses only the base helper set (map lookup,
ktime, ringbuf — each read back from the 5.15 source) the tap
itself proves. The record schema is structural zero-leakage —
(timestamp, kind, value); there is NO keycode field, so a control
notice cannot carry key data, and the events ring keeps its own
record shape and its own pin. The mechanism is floor-legal by construction: the relay
is a socket filter because aya loads the `socket` section natively
and the 5.15 floor's test-run answers it (a SYSCALL program needs
the sleepable flag and no aya section maps to it; a KPROBE answers
ENOTSUPP — both recorded in the PROOF 7 chain), and a queue/stack
map was refused because 5.15 has no `map_poll` for them — polling
one answers EPOLLERR, a busy loop wearing a wake's clothes. The
wake that changes nothing says "authority held" out loud: a notice
is a hint, the map is the law. Proven in the e2e guest (PROOF 8):
a real `volume` verb lands with a kernel-timestamped wake row, the
same-value verb produces the authority row, and the harness's
notice-drive lane (relay runs, ring wakes, mmap drain echoes the
frame) introspects the bytes. Files of truth: `src/ebpf/notice.rs`
(the schema, compiled by both crates), `ebpf/src/main.rs` (the
relay), `src/ebpf/bpf_syscall.rs` (the publish),
`src/commands/player.rs` (the two-fd watcher), and
`scripts/audit/harness/src/noticedrive.rs` (the kernel-side lane).

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

night-frontier-2B turns the stamp into a reproducible-build
attestation: the timestamp honors `SOURCE_DATE_EPOCH` (the
reproducible-builds standard — decimal digits only, a malformed pin
is visibly ignored per the standard's own fallback law) and the
clock that answered is stamped alongside (`ZYLAXION_EPOCH_SOURCE`);
the dependency set rides the binary as a lockfile fingerprint
(`ZYLAXION_LOCK_HASH`, FNV-1a 64 over the `Cargo.lock` bytes — the
byte-stream twin of the audit estate's fnv1a64, still std only). The
doctor's BUILD block names both facts; `-V` stays frozen. The proof
instrument (`scripts/dev/verify-reproducible-build.sh`) builds the
pro alias twice from one commit in two separate target directories
with the epoch pinned to the HEAD commit's own timestamp: on the A/B
toolchain the two binaries came out **byte-identical** (sha256
`007cfa43…` on both), the full reproducible-builds property, not
just the stamps — and the supermassive claims lane audits the
attestation every push.

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
"rezky_nightky (oxyzenQ)" ships only as splitmix64-XOR-masked
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
`zylaxion sig` walks the five verification depths (runtime identity,
layer binding, anchor integrity, the running image's own self-hash
fingerprint — a fingerprint by law, it cannot embed its own digest —
and, since night-depth-hardening-Z1 D2, the tap-object row: the
embedded eBPF object re-hashed against the build-time stamp
`ZYLAXION_TAP_OBJECT_B3`, the ring-0 asset pulled inside the
self-verification circle, a post-build patch refused by name with both
digests; the zejtron night-frontier-Z5 shape) and exits 2 on any
tamper. Files of truth: `src/info/signature.rs`,
`src/commands/sig.rs`, and
`scripts/release/generate-provenance-words.py`; the stamping lives in
`build/tapstamp.rs`; pinned by
`test/info/signature_tests.rs` and `test/commands/sig_tests.rs`, and
proved end to end on a real patched binary by
`scripts/audit/tamper-sig-probe.sh` (the CI check job's tamper-sig
probe leg).
night-depth-hardening-Z1 deepens the matrix with the process-identity
depth (D4): the `Dragon Guard - Reproducible Release Chain` lane builds
the static musl v3 shape on two independent hosts from one pinned
recipe (`scripts/release/repro-build.sh`, toolchain + epoch + redacted
paths + locked deps) and `cmp`s them byte-identical in CI — the
published hash becomes the hash anyone can rebuild from source, so the
identity the matrix seals is also the identity the PROCESS produces.

**39. The kernel matrix proof — the "5.15+" floor measured, not
claimed.** The floor question: the tap's kernel contract (the fentry
`attach_btf_id` resolved at load time from the target kernel's own
BTF) was verified against real BTF snapshots of the three LTS lines —
Ubuntu jammy 5.15.0-97, Debian bookworm 6.1.0-50, Arch LTS 6.6.72-1 —
each extracted from a sha256-pinned distro kernel image (the pin
table is the only legal way the artifacts change), decompressed to
the vmlinux ELF with the kernel's own extract-vmlinux shape, carved
to the raw `.BTF` section with objcopy, and verified through the
loader's own aya-obj path: object parse, the measured CO-RE posture
(`has_btf_relocations() == false` on the shipped object), the
relocation chain clean, and `input_event` resolving to a BTF `Func`
id — the exact lookup `FEntry::load` performs at attach. Map-fd
relocation stays a live-kernel fact (the snapshot cannot mint fds);
the harness records the map inventory and the attach chain owns that
verdict. The driver (`scripts/audit/zactrix-kernel-matrix.sh`) and
the harness mode (`--btf-matrix`) run on every push in the
supermassive claims lane (artifacts cached, sha256 verified on every
run); the matrix read 3/3 PASS on the A/B machine and the live host
BTF (5.10) rides as instrument evidence only — below the supported
floor, never counted. Files of truth:
`scripts/audit/zactrix-kernel-matrix.sh`,
`scripts/audit/harness/src/btfmatrix.rs`; the measured matrix is
recorded in `docs/KERNEL_COMPATIBILITY.md` and
`docs/CO_RE_PORTABILITY.md`. night-runner-1 completes the same law on
the runtime half: the Supermassive E2E KVM matrix grew the LTS-span
witness seats (the 6.1 Debian bookworm and 6.6 Arch LTS pinned
artifacts, the 6.8 noble GA frozen pocket) beside the 5.15 floor and
the archive head, each booting the real kernel family and running the
whole battery with the exact-family witness — the pending runtime
records of the kernel checklist now live as CI verdicts, not doc
claims.

**40. The golden render corpus — the freeze as a byte gate.** The
freeze (entry 31) names the admission law in prose; the corpus makes
it mechanical: the committed per-preset x per-rate render baseline
(the rates-parity corpus — 24 presets x 5 rates plus the per-preset
summary rows, one corpus, no duplicate baseline) is re-rendered
through the REAL VoicePool path and compared row by row, keyed on
(preset, rate), with a machine-readable verdict naming every delta
(`GOLDEN VERDICT: baseline=144 live=144 unchanged=144 changed=0`,
every CHANGED/MISSING/ADDED cell named with its was/now pair). The
gate's honesty is two-tier, because the measured boundary is real:
the hash rows are machine fingerprints (the init-time transcendental
math — per-target libm and CPU dispatch — the same boundary the
rates-parity lane documents), so byte identity is gated STRICTLY only
where the fleet is one CPU family — the arm64 native lane (its
committed baseline holds byte-identically, proven by the parity lane
in the same workflow) and the owner's A/B machine — while the
heterogeneous x86 claims lane runs the structural gate (missing or
added rows fail; the measurand's shape is machine-independent) with
every byte delta named and recorded. A clean strict verdict is the
sentence "the sound IS the bytes" holding as a gate: any drift — a
coefficient, a clamp, a table — moves named rows and demands the
documented-cause re-baseline law, from any hand, including a
post-acquisition engineer's. The gate never stamps the baseline
itself; stamping stays the rates-parity lane's logged-run contract.
File of truth: `scripts/audit/zactrix-golden.sh`; the corpus:
`scripts/audit/rates-parity-baseline.txt` — ONE corpus since
night-frontier-Z5 (entry 47): the machine-independent bytes retired
the per-arch split, and the byte gate is strict on every lane.

**41. PROOF 7 — the tap's in-kernel cost, measured by the kernel
itself.** The README's "~1-2 µs per keystroke" and "the whole cost is
one branch" claims stop being marketing: the harness's `--prog-run`
mode drives the PRODUCTION tap object (the ebpf-prebuilt bytes the
binary embeds, blake3-pinned) with synthetic input_event arguments in
a real x86_64 pt_regs, and the kernel reports the duration per class
— mapped press, release, autorepeat (the value-2 early exit), unmapped
BTN_* (the keymap death), and the disabled gate. The loader's own
path is the lane's path: the first CI attempt named the wall a generic
tool cannot cross (the aya-ebpf object carries LEGACY map definitions,
and libbpf v1.0 / every modern bpftool refuses to open the object
before any measurement), so the mode speaks the parser the loader
speaks — aya-obj parses the object, real maps are created from the
parsed definitions through raw `bpf()` syscalls, the map-fd relocation
walks the loader's own step, and the kprobe body loads unattached.
The gate is the RELATIVE law the architecture claims (gate-off,
unmapped and autorepeat all measure below the full press path); the
absolute ns/call numbers are recorded as measurements with the repeat
count in the row, never gated to a fabricated bound (the runner's CPU
is not the buyer's). A witness closes the gimmick door: with the gate
on, the measured presses move the tap's own in-kernel press counter
(STATS_TOTAL) inside the freshly created maps — the measured path is
the real render path, not an early exit. The kernel contract is
stated honestly: a KPROBE program answers ENOTSUPP to TEST_RUN on
every mainline kernel (the round-6 run recorded the refusal — there
is no `bpf_prog_test_run_kprobe`), so the mode loads the SAME bytes
as `BPF_PROG_TYPE_SYSCALL`, the one type whose TEST_RUN the kernel
supports since the 5.15 floor and the mechanism the kernel's own
selftests time programs through — every at-floor kernel measures,
nothing is skipped. The lane runs in the e2e micro-VM lanes (root, a
real kernel), the harness binary riding the payload artifact. Files
of truth:
`scripts/audit/harness/src/progrun.rs` (the mode),
`scripts/audit/zactrix-prog-run.sh` (the host wrapper), the guest
wiring in `scripts/ci/supermassive-init.sh`; night-frontier-Z5
extracted the lane head into `prepare_tap`, which entry 48's corpus
mode shares.

**44. The proven datapath — bounded model checking over the pure
laws (night-depth-hardening-2).** Unit tests sample; this lane
quantifies. Three pure law functions prove over their WHOLE
reachable input spaces under Kani (bounded model checking — the
verifier lane `Dragon Guard - Kani`, the estate's 10:00 WIB clock,
Sunday plus manual dispatch): the volume clamp cell, where every f32
the domain can name either clamps into [-100, 100] exactly with a
lossless bit round-trip or, non-finite, falls back to the compiled
default; the gain law, where the untrusted control-map percent — the
whole u32 space, the map is not input — cannot produce NaN,
negatives, overflow, or a quieter render at a louder percent; and
the noise mapping, where every u32 bit pattern the PRNG produces
lands inside [-1, 1], finite. The harnesses compile under cfg(kani)
alone — invisible to every build, test, and lint lane; the cfg is
declared in Cargo.toml so the rustcfg check stays silent. Loom is
honestly refused at this frontier: the SPSC ring is the third-party
ringbuf crate (nothing of ours for a model to walk) and the one
first-party lock-free surface is the single-atomic volume cell this
lane already proves — a loom model over one atomic word would prove
what the hardware contract already grants; a loom lane rides a
first-party lock-free structure if one ever lands. night-frontier-Z6
(entry 49) widened the proven surface to the control plane's integer
laws this entry left measured. Files of truth:
`src/engine/kani_proofs.rs` (the harnesses), `.github/workflows/
kani.yml` (the lane, the action SHA-pinned), `Cargo.toml` (the cfg
declaration).

**45. The unsafe audit's score law (night-depth-hardening-3).**
Every unsafe block names its own safety argument, and the discipline
cannot rot back: `clippy::undocumented_unsafe_blocks` rides the
three crate roots at deny, and all 64 unsafe blocks across the
estate carry honest per-site `// SAFETY:` comments — the syscall
attr laws and fd ownership in the bpf syscall layer, the FP
control-register asm in the denormal policy, the ringbuf record
casts, the pipe and sigaction wires, the test tree's env and
self-wake blocks. Comments only, zero logic moved — the audit's
score is 100% because every block justifies itself, not because any
was deleted. Files of truth: `src/main.rs`, `ebpf/src/main.rs`,
`scripts/audit/harness/src/main.rs` (the three deny laws), and every
`// SAFETY:` line under `src/`, `test/`, `ebpf/src/`, and
`scripts/audit/harness/src/`.

**46. The last timer dies — the parked player and the evented stop
(night-frontier-Z4).** The arc's final demotion: entry 29 took the
idle burn 1.9% → 0.035% (ten timer wakeups a second), and this entry
takes it to ZERO — the render loop's silent wait is a blocking select
over [the event channel, the stop lane] with no timeout, the bridge's
events-ring poll is a FOREVER block (its one exit lane is a wind-down
pipe the process writes at teardown), and the watcher's poll lost its
timeout too. The physics: a parked thread is asleep in the kernel
until a channel op or an fd event lands — no atomic store can wake
it, so every stop decision became a message: the watcher forwards one
`ControlStop` on a channel (the cleared enabled gate, the read-
failure verdict, the graceful signals, the wind-down), the graceful
handler — the house's own sigaction lanes now, signal-hook left the
dependency tree (12 → 11 direct dependencies, 129 lockfile crates) —
writes one byte to the control wake pipe so SIGTERM wakes the parked
watcher, and the post-run teardown pokes both parked threads (the
flag is the decision, the byte is the wake) so no join can hang. The
one timer that survives is the watcher's 60 s backstop, and it arms
ONLY when the poll set is empty (both the notice ring and the wake
pipe failed to open — an exotic sandbox): a timer is the only
remaining wake channel when both evented lanes are dead, and `off`
must always land — a backstop, not a poller (one wake per minute,
one map read each). The tamper posture is UNCHANGED and honestly so:
the held map fds outlive an unpin (the inode lesson the orphan sweep
learned), so a hand-removed pin set was never detectable from the
player's seat — Z4 changes the cadence of a silence that already
existed, and `attach` + `off` recovers it as before. The evidence is
mechanical: the in-tree `idle_park_emits_zero_wakeups` test counts
the parked thread's context switches from /proc (zero over a 2 s
window), `control_stop_wakes_the_parked_loop` proves the message
lane wakes the park inside a 200 ms bound (the 300 ms disconnect
fallback beneath it fails loudly, never hangs a runner),
`idle_cpu_sanity` phase 1 witnesses the same zero on the real loop
(getrusage 0.004% of one core on the audit machine — the full
progression 1.131% → 0.035% → 0.004%, and the last step is zero
events, not a smaller percent), and the E2E battery grew the
parked-player witness leg (`scripts/supermassive/sm_idle.py`, its
own no-mercy module): attach, on, a 20 s silent window, and the
per-thread deltas — render +0, bridge +0, watcher ≤ 1, the player
alive, and `off` still landing through the evented stop — on every
micro-VM kernel leg. The engine freeze is untouched: the render
bytes are exactly the pre-Z4 bytes (the golden corpus's structural
gate: missing=0, added=0, the byte-delta profile identical to the
pre-Z4 tree on the same machine — the documented libm/CPU dispatch
boundary, not a render change). Files of truth: `src/core/mod.rs`
(the park, the ControlStop contract), `src/commands/player.rs` (the
bridge's forever poll + wind-down pipe, the teardown pokes),
`src/commands/control_watch.rs` (the forever poll + the 60 s
backstop + the ControlStop forwards), `src/session/signals.rs` (the
graceful sigaction + the wake byte), `src/session/wake.rs`
(`installed_wake_write_fd` + `poke`), `examples/idle_cpu_sanity.rs`
(the phase-1 /proc witness), `scripts/supermassive/sm_idle.py` (the
E2E parked-player witness); pinned by `test/core/core_tests.rs`
(`idle_park_emits_zero_wakeups`, `control_stop_wakes_the_parked_loop`)
and `test/commands/help_tests.rs` (the retired-heartbeat pin).

**47. The certified bounded-domain transcendental lane — the libm
leaves the audio path, the bytes go machine-independent
(night-frontier-Z5).** The boundary this entry closes is the one
entry 1 (3C) and entry 40 measured and deferred: the init-time
transcendentals resolved through per-target libm, so the golden
corpus's hash rows were machine fingerprints and the byte gate could
only hold on a fleet-homogeneous lane. The lane replaces every libm
transcendental on the audio path — the init anchors' tan, exp, powf,
cos, and sin (ten call sites) and the render path's per-sample f32
tan (click and spring travel, until the frozen-coefficient law
freezes them) plus the housing fade's per-sample f32 exp (three
sites) — with pure IEEE-754 f64 arithmetic: Taylor polynomials on
CLOSED, architecture-locked domains (exp on [-64, 0], ln on (0, 1],
sin/cos on [0, 4*pi] through a three-part Cody-Waite quadrant
reduction, tan on [0, 0.45*pi] as the certified ratio, and the
decay-law pow with its exact libm edges — pow(c, 1) == c keeps the
44.1 kHz reference bit-identical), evaluated by Horner whose
operation order Rust never reorders or contracts into FMA, so the
returned bits are identical on x86-64-v3, x86-64-v4, arm64, and
every glibc version, forever. The certificate is arithmetic, not
trust: the generator (`scripts/engine/generate-detmath.py`,
stdlib-only) computes every bound as an EXACT Fraction — the
truncation term plus a conservative Horner rounding accumulation,
rounded OUTWARD — and emits the tables plus golden bit vectors the
test tree pins bit for bit (`test/engine/detmath_vectors.rs`; the
generator's own simulation of the algorithm is the golden master).
The bounds ride the tables under cfg(test) — a certificate exists
exactly where it is enforced — and the domain guards refuse loudly
outside the certified domain: unreachable by the clamp laws, kept as
the house's clear-refusal tripwire. Measured on the documented-cause
re-baseline: the corpus's measurands are unchanged per row (voice
lifetime, ring decay, tail endpoint identical across the whole
24 x 5 grid), every preset keeps its audit tier, the in-run and
cross-run renders are byte-stable, and the f32 anchors match the
libm oracle's anchors inside one spacing on the physically
meaningful band — the quadrant boundary's clean-zero exception is
bounded by the emitted RED_RHO certificate and pinned as such. The
corpus, the parity lane, and the golden gate all upgraded with it:
ONE baseline serves every architecture, the arm64 runner proves
itself against the x86-committed bytes, and the byte gate is strict
everywhere (entries 40 and 41 name the upgraded lanes; the audit
record's closure section lives in docs/ZACTRIX_AUDIT.md). Files of
truth: `src/engine/detmath/mod.rs` and `tables.rs`, the generator,
`test/engine/detmath_tests.rs` + `detmath_vectors.rs`, and the
updated call sites in `src/profiles/mechanical/{init,render}.rs`;
the freeze admission rides the documented why in
`src/engine/mod.rs`.

**48. The tap-behavior golden corpus — the kernel side IS the bytes
(night-frontier-Z5, PROOF 9).** Entry 40 pinned the userspace render;
entry 41 measured the tap's cost; the kernel half's OBSERVABLE
BEHAVIOR had no corpus. This lane pins it: the production tap object
through the PROOF 7 lane's own head (`prepare_tap` — aya-obj parse,
fresh maps from the parsed definitions, the loader's map-fd
relocation, a raw bpf() load as the syscall type), driven through a
scripted event sequence via BPF_PROG_TEST_RUN, with the events ring
drained through the generic consumer lane
(`scripts/audit/harness/src/ringdrain.rs`, the kernel's ringbuf mmap
law single-sourced with the notice-drive lane). The golden contract
is COMPILED — the tap's own semantics read off `ebpf/src/main.rs`:
gate-off silence (zero records, zero counters), the keymap death,
the autorepeat filter, the exact record order (press, release,
burst), the ring saturation with the capacity MEASURED in-run
against its byte ceiling (the first guest leg caught the derived
constant guessing the kernel's usable-byte math wrong — 1792 pinned,
2047 measured, against a 2048 ceiling: the tap was right, the golden
constant was not — so the contract pins the BEHAVIOR: fill-until-full,
drop-after, ring-recover, with the overflow guaranteed by arithmetic
since 2203 x 32 bytes exceed the reported 65536 on any kernel), and
the counter law — stats count EVERY press because the fetch-add
rides BEFORE the ring reservation, so a dropped event degrades the
sound, never the telemetry. Timestamps are masked (the kernel clock
is not behavior); the record shape mirrors the tap's 24-byte
KeyRingEvent the way the pin paths are mirrored. A kernel or tap
change that moves one tuple or one delta fails with the delta named;
a capacity move lands as the named measured row. The lane needs NO attach
and leaves NO residue (fresh maps, the process exits, the kernel
reaps them); it runs beside PROOF 7 in the e2e micro-VM guest and
honestly skips elsewhere. Files of truth:
`scripts/audit/harness/src/tapgolden.rs` (the mode),
`scripts/audit/harness/src/ringdrain.rs` (the shared drain),
`scripts/audit/zactrix-tap-golden.sh` (the host wrapper), and the
guest wiring in `scripts/ci/supermassive-init.sh`.

**49. The proven control plane — the measured integer laws become
proved (night-frontier-Z6).** Entry 44's verdict carried an honest
remainder: the errno classification table (entry 28), the orphan
decision ladder (entry 25), the privilege polarity table (entry 26),
the no-config preset resolution (entry 27), the pool's NaN-silence
mix net, and the envelope decay chain were pinned by tests — sampled,
not quantified. This lane closes that gap under the same verifier
(sixteen new harnesses in `src/engine/kani_proofs.rs`, the same
`Dragon Guard - Kani` lane): the marker wire format round-trips
inside the pinned 1..=9 band and refuses every code outside it, over
the whole u32 space; the three errno classifiers classify the WHOLE
i32 errno domain exactly per the kernel's own table (EPERM/EACCES
privilege, ENOENT the missing symbol, EINVAL the refusal, everything
else the honest Unknown); the errno text table is total and
non-empty over i32; the verifier-log tail stays bounded (at most
`keep` lines, each capped) over bounded symbolic logs; the orphan
ladder is total and shape-true over all 256 stage values, every arm
carries a reachability witness (no orphan decision), every stage at
or above 2 collapses onto stage 2's behavior (a corrupted stage byte
cannot invent a fourth step), and the reaper's identity gate breaks
on ANY single flipped byte of a matching cmdline (a map-driven
/proc retarget cannot mutate its way past the exact-match law) with
unreadable shapes refusing by construction; the privilege table
matches the audit over the whole fact space — root refused at the
network and session surfaces, REQUIRED at attach (the mirror), detach
refusing exactly when a non-root caller cannot complete either
removal half, every fired wording non-empty and carrying the sudo
tip; the no-config preset resolution is total over the whole u32
index domain — in range it IS the registry entry, out of range every
index collapses onto the one compiled default, the profile stays
finite and inside the clamp bands, and the path never panics; the
pool's mix net is proved END TO END through the real pool code: two
live voices render stereo pairs from the render-reachable band into
a gain from the volume cell's proven band, and the sample the user
hears is always finite and always inside [-1, 1] — the worst the
render can do is silence, held to the reachable space (the verifier
refuses to reason through a NaN intermediate, so the net's branch
for the impossible-under-the-clamp-laws states stays pinned by the
pool test tree, named honestly in the harness); and the envelope
decay chain is monotone
non-increasing for every step below 1.0, strictly decreasing above
the denormal-glue floor inside the certified device domain (the
largest reachable renormalized coefficient is 0.9999 raised to the
smallest rate ratio the documented device span allows, 44100/384000
— a f32 constant with its derivation in the harness), and the
voice-off band is ABSORBING: an envelope below the threshold can
never resurrect through decay arithmetic. The honest SMT limit
holds and is the reason the order mattered: functions whose behavior
rides a transcendental stay outside the solver — and
night-frontier-Z5 moved the transcendentals to certified
build-time tables precisely so the decay chain THIS entry proves
became pure mul/add. The four dense-sweep detmath tests stay
miri-gated and the harnesses stay cfg(kani)-invisible. Files of
truth: `src/engine/kani_proofs.rs` (all sixteen harnesses), the
widened `pub(crate)` helpers in `src/ebpf/attach_error.rs` (the
errno text and the tail, proved through). The lane's compile gate
caught its first toolchain skew on the first full CI verdict after
Z6: the verifier's pinned nightly reads inline format captures as
literal strings — which leaves the mix-net harness's channel binding
unused under the lane's deny-warnings — so the harness's messages
carry explicit format args and three dead imports left the module,
with zero change to any harness's proof.

**50. The C-state residency witness — the parked loop lets the core
rest, and the rows say so (night-frontier-Z6).** Entry 46 (Z4) proved
the parked loop costs 0.004 percent of a core with ZERO timer
wakeups; the physics half — the core actually FALLS into the
machine's idle states and STAYS there — was the claim's unmeasured
half. Two witnesses, one law, each on the stage that can honestly
see it. The E2E guest battery (sm_idle) brackets its existing silent
window with two snapshots: the kernel's own idle-time accounting
(/proc/stat — always present) gates at 90 percent of the window (a
spinning poll eats the delta and fails; the parked loop rests), and
the cpuidle residency rows (per-state time deltas, aggregated across
cpus) are RECORDED, never gated — a micro-VM guest exposes whatever
idle states its kernel and machine model carry, and the host's real
deep C-states live above the guest's view entirely, so the absent-
sysfs case is a provenance NOTE naming the host instrument. The
first CI watch answered the original honest skip with the strict
lane's own verdict — "1 probe(s) skipped and this lane accepts no
skips" failed all five e2e specs, the gate doing exactly what it was
built to do — and the row was reclassified: recorded where the sysfs
exists, provenance where it does not, never a skip. The host
instrument (idle_cpu_sanity, the owner-machine benchmark) gained the
same witness in its phase-1 park: cpuidle sysfs snapshots bracket the
silent window and print per-state residency deltas — the measured
first run read `haltpoll idle +3918 entries, +19672367 us` alongside
`POLL +4741 entries, +149002 us` on the two-vCPU sandbox: both cores
spent essentially the whole window in the halt state while the parked
loop woke zero times. The claim the battery now carries: zylaxion's
presence in a silent session is not merely cheap, it is INVISIBLE to
the power profile — the cores rest in their idle states for the
whole silence, witnessed by the kernel's own counters on both stages.
The gate stays the CPU and wakeup bars (Z4's); the residency is the
row, recorded where it can be honestly seen. Files of truth:
`scripts/supermassive/sm_idle.py` (the guest witness: the idle-time
gate and the residency rows riding the same silent window),
`examples/idle_cpu_sanity.rs` (the host witness: the cpuidle
snapshot, the delta rows, the honest skip).

**51. The release-impact physics law — one model, two impacts
(night-frontier-Z7, verdict LEARN).** The release event already flows
end to end: the tap emits the record with value 0, the pool releases
the voice, the two-millisecond ramp ends the press — and all of that
energy is spent on one job, stopping. Real switch physics says the
upstroke is a SECOND impact: the click jacket's reset on a rubber
dome, the return ping of a buckling spring — audible, distinct, and
physically the same mechanism as the press, measured at another
point of the same travel. The honest world-state: sample-based tools
ship separate release samples; what nobody ships is a reset COMPUTED
from the same law as the press — one stage machine, two excitations,
the second's window inverted and its energy scaled from the
press-to-release travel time the tap timestamps already carry. The
deepening would synthesize the release as that second excitation
from the SAME stage machine — not a second sample, not a new
feature, and the kernel contract does not move one byte. The verdict
is LEARN, deliberately, with every reason named: the lane touches
the frozen Zactrix engine (entry 31), it forces a re-baseline of
every preset through the documented-cause law, and the final judge
is the owner's ears — CI has no ears, and no CI-verifiable metric
can speak for the perceptual depth the lane exists to add. The
execution path stays filed for the day the realism lane opens: the
harness metric first (the energy onset inside the release window),
then the owner's ears, then the re-baseline with its documented
cause.

**52. The 1/f human-motor fluctuation law — the red thread in the
randomization (night-frontier-Z7, verdict LEARN).** Entry 5's
micro-randomization is independent per keystroke; human motor
control is not. The psychophysics literature on tapping and typing —
long-range correlations in the intervals, the ~1/f slope of the
power spectrum, red noise carried across seconds — is decades deep,
and no keyboard synthesizer implements it: the randomization every
tool ships is white at exactly the level where humans are red. The
deepening: an integer-only 1/f filter — zero allocation, a handful
of integer operations PER TRIGGER, never per sample — modulating the
amplitude and timing drift so a long session's envelope series
carries a power-spectral-density slope near -1, the statistical
signature of human motor fluctuation at the envelope level. The
proof is physical, not taste: a harness measures the PSD slope of
the synthesized series and the claim becomes a number, the same
measurand class the PROOF harnesses already emit. No CLI, no kernel
change — but like entry 51 it touches the frozen engine and the
re-baseline law, and the ears stay the judge; verdict LEARN, the
ideal companion of entry 51 if the realism lane ever opens.

**53. The lazy loading helix — the indeterminate wait narrates
itself (night-ux-orion-1; `src/output/spinner.rs`, wired into
`src/commands/update.rs`).** The hole: an indeterminate wait and a
hang were indistinguishable — the update fetch blocks up to its 15 s
curl ceiling with a silent terminal, and the Ctrl+C a frozen-looking
terminal invites is a bug of perception. The close: a loading lane
whose first frame lands only after the operation crosses 100 ms (a
fast path never draws — silence stays byte-for-byte), then shows the
DNA helix, two braille strands running the same ten-frame sweep in
opposite directions — `HELIX[i]` pairs the sweep's `i`-th cell with
its `(9-i)`-th, the strands never collide because `i = 9 - i` has no
integer answer, and the two anti-parallel streams are the fetch
itself: the request out, the answer back. The fallbacks are
detection, never flags: a NO_COLOR terminal or a non-UTF-8 locale
drops automatically to the rotating ASCII dash (`- \ | /`), the
paint rides the calm-grey tier through the one-gate color law (the
crate's documented truecolor-grey approximation, plain in Mono), and
the line rides the family's bracket grammar with the \r clear
contract, and a piped stderr stays silent — all pinned pure (frame
tables, style ladder, lazy slot clock) in
`test/output/spinner_tests.rs`. Zero new dependencies: std threads,
two AtomicBools (the stop flag and the drawn flag the clear rides),
and tables of literals. The live bytes are witnessed too: the
`sm_ux.py` loading-lane probes run `--check-update` through a real
pty on both seats (the plain dash floor and the braille helix), pin
stdout spinner-free, and require the full-width clear after the last
frame — the verdict never lands on a line the spinner left dirty.

**54. The loading helix's estate — the local large-data loads
narrate themselves (night-ux-orion-2;
`src/commands/sound/lifecycle.rs`, `src/commands/info/doctor.rs`).**
The hole: orion-1 closed the perception bug on the one networked
wait, but the estate's three local waits stayed silent — `on`'s
settle loop polls the control map while the detached player boots
its engine, opens the audio server and voices the preset; `off`'s
stop loop waits out the fade the player's watcher runs after the
gate clears; the doctor's audio probe rides the device open and the
audio server's own handshake. On a slow device, a busy host or a
wedged audio server each crosses the 100 ms lazy threshold and the
terminal sat frozen-looking — the Ctrl+C a silent shell invites
kills a healthy spawn, a clean fade, a running diagnosis. The close
rides the SAME machine, byte for byte: the same Spinner, the same
HELIX table, the same lazy threshold, the same style ladder and
clear contract — the helix seat on all three (`starting the sound
engine`, `stopping the sound engine`, `probing the audio device`), a
fast path never drawing. The one structural law the estate adds:
the arm/finish bracket wraps EXACTLY the wait, and no exit lane may
print or leave from inside the bracket — the settle loop lands its
three exits as a verdict enum, the stop loop's budget verdict breaks
out as a flag, the doctor collects the probe into a value first —
so the machine finishes and erases its drawn line before the first
verdict byte prints. Zero new dependencies, zero new CLI surface,
zero new machinery. The live bytes are witnessed by the
`sm_ux.py` loading-estate probes: the piped byte-silence law on all
three seats, and the pty contract (frame legality against the helix
seat, the full-width clear, the clean stdout) — frames drew or did
not, both green wherever the suite runs.

## The test tree that pins the ledger

The ledger is not honored by prose alone: 39 test files carrying 308
test functions pin the entries above, wired into the crate through
`#[path]` modules. The family map, so an auditor can walk it in one
pass:

- `test/profiles/` (mechanical, profiles, registry) — the Zactrix
  stages, the TPT chain, the housing window, the rate-normalized
  decay law, micro-randomization, the decoupled noise streams, the
  pan table, interval attenuation, velocity scaling, and the registry
  clamp contract (entries 1–10, 14).
- `test/engine/` (pool, render, freeze, detmath) — oldest-first
  stealing, the smooth release ramp, the clamp and NaN silence net,
  the engine freeze declaration, and the certified transcendental
  lane's vectors and certificates (entries 9, 11, 12, 31, 47).
- `test/core/` — the Orchestrator: the pan law, the timestamp lane,
  the adaptive wait, the ring bridge behavior, and the Z4 park
  witness (entries 7, 8, 15, 29, 46).
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
render path; the saturation-boundary rows of entry 43 re-run with
`cargo run --release --manifest-path scripts/audit/harness/Cargo.toml
-- --fold-proof`; the certified lane's vectors and certificates verify
with `cargo test detmath` and regenerate through
`scripts/engine/generate-detmath.py`; the tap-behavior corpus runs
`scripts/audit/zactrix-tap-golden.sh` (root, beside PROOF 7 in the
e2e guest); the proven control plane verifies with `cargo kani`
(entry 49's sixteen harnesses, the verifier lane); the C-state
witness rides the silent window of the parked-player battery
(sm_idle) and the owner-machine benchmark (idle_cpu_sanity);
`zylaxion doctor` prints the kernel report the attach
gate consumes; `zylaxion status` reads the TAP_KIND slot the loader
wrote; the proven-datapath harnesses of entry 44 verify under the
Kani lane (`cargo kani` — the Sunday 10:00 WIB schedule or a manual
dispatch from the Actions tab); the unsafe audit's score law of
entry 45 is `cargo clippy --all-targets -- -D warnings` itself; the
parked-player law of entry 46 is `cargo run --release --example
idle_cpu_sanity` (phase 1 prints the /proc wake count — the zero —
and the CPU figure) and, on a root lane, the E2E parked-player
witness leg (`scripts/supermassive/sm_idle.py` through the full
battery). Entries 51 and 52 carry no instruments by their own
verdict: LEARN is a filed decision, and the proof obligation arrives
only if the realism lane opens. Where a number in any other doc
disagrees with the code, the code wins — and the doc gets the PR.

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
