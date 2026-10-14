<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

# The Security Audit — night-private-research-2xblade-3.6

One question, answered with evidence: what is the attack surface of
zylaxion, and does any part of it expose the user to harm that the
zero-leakage posture in the README claims is absent?

The README makes four strong claims:

  A. Zero-leakage: keystrokes never leave the process.
  B. No daemon, no IPC, no socket, no network inbound.
  C. The kernel tap is passive.
  D. The only outbound network call is `--check-update`.

This audit verifies each claim against the source, names every
counterexample, and grades one finding per dimension:
NONE / LOW / MEDIUM / HIGH / CRITICAL / INCONCLUSIVE. A fix in an
audit pass is reserved for specific, reproducible vulnerabilities
within 100 LOC across 2 files; everything larger is a named residual
for a follow-up pass.

## Method and sandbox honesty

The audit read the whole security-relevant tree end to end:
`ebpf/src/main.rs`, `src/ebpf/` (loader, bpf_syscall, pin, teardown,
embedded, args), `src/session/` (kernel, privilege, signals, orphan),
`src/commands/` (player, sound, attach, update, stats, info),
`src/core/`, `src/engine/`, `src/profiles/` (render and pool paths),
`build.rs`, `build/ebpf.rs`, `src/info/build.rs`, `Cargo.toml`,
`Cargo.lock`, `scripts/dev/bootstrap-ebpf.sh`, `scripts/build.sh`,
`scripts/gate-keepers.sh`, `.github/workflows/release.yml`, README,
SECURITY.md, docs/VERIFY_RELEASE.md.

Evidence greps quoted below were run against this tree at commit
`084ce54` (2xblade-3.5 record). The sandbox has no root, no bpffs
write access, no bpftool, and no audio device. Every measurement that
needs those is marked INCONCLUSIVE with the reason and what would be
needed to conclude — an honest gap, not a silent pass.

---

## Dimension 1 — Keystroke confidentiality (claim A)

### Surface

The path from the eBPF tap to the DSP engine to the audio buffer:
every struct that carries a scancode-class field, every map, every
log line, the panic handlers, and the buffers a core dump would
carry.

### Threat model

An attacker with read access to process memory, the log stream, the
syscall trace, or a core dump reconstructs typed text from the
engine's outputs.

### Evidence

**The dataflow, end to end.** The tap reads the four arguments of
`input_event()` (`src/ebpf/args.rs`), filters `EV_KEY` with
`value <= 1` (autorepeat dropped in-kernel, `ebpf/src/main.rs:237`),
checks the `control` gate, looks the keycode up in `keymap`
(unmapped keys die in the kernel), then publishes one 24-byte
`KeyRingEvent { ts_us, keycode, value, slot }` to the ring buffer.
The player bridge (`src/commands/player.rs:233`) casts the record to
`KeyEvent { scancode, pressed, timestamp }` and hands it to the
orchestrator. `src/core/mod.rs:487` uses the scancode exactly three
ways: `scancode_to_pan` (a stereo position), `pool.trigger` (the
`KeyTrigger` carries it), and `pool.release` (a match key). The
render path ignores it by contract
(`src/profiles/mechanical/render.rs:21`, `let _ = event.scancode`),
and the pool zeroes `voice.scancode` the moment a voice deactivates
(`src/engine/pool/mod.rs:217`, the v10.2.0 dragonzen-audit P4 rule).
Nothing translates a scancode into a letter anywhere in the tree.

**The stats map layout.** `stats` is `Array<AtomicU64>` with 257
slots (`ebpf/src/main.rs:176`): per-keycode press counters in
0..=255 and a total at slot 256, accumulated with `BPF_ATOMIC`
fetch-add. Aggregate counts only — no per-press timestamps, no
sequence storage. The only sequence-capable carrier is the ring
buffer, which is transient process memory drained in microseconds.

**The log audit.** The production-path grep:

    rg -n 'scancode|keycode' src/ --type rust \
      | rg 'log::|println!|eprintln!'

returns exactly one hit: `src/commands/stats.rs:74`, the
`status --verbose` telemetry render, which prints the per-keycode
aggregate counts with human-readable names ("keycode  30 (A)").
That is the disclosed typing-telemetry lane — aggregate counters the
README documents ("a frequency histogram cannot be replayed into
typed text"), rendered on the operator's own terminal on request.
No other log, warn, error, debug, or trace line in the tree names a
keycode or a scancode. The player logs pids, preset indexes, volume
percents, sample rates, and idle seconds — never key identities.
The single `log::debug!` in the tree (`src/core/mod.rs:364`) prints
"no key events in Ns — idle": timing metadata about absence, not
content.

**The panic handlers.** The kernel-side handler is `loop {}`
(`ebpf/src/main.rs:298`) — it prints nothing, ever. The userspace
panic hook (`src/main.rs:76`) prints location and the payload
string only; no panic or expect message in the tree formats an
event field into its text (all payloads are static strings).

**The core-dump posture — the honest answer is yes.** A keystroke
lives, briefly, in buffers a core dump would carry: the mmap'd ring
pages (up to 64 KiB of recent records), the crossbeam channel queue
between the bridge and the render loop (usually empty within one
loop iteration; unbounded under an audio stall), and the active
voices' `scancode` fields (zeroed at release). A core dump of the
player is therefore a recovery carrier for a bounded recent window
of scancodes plus timestamps. Scancodes are not letters — recovery
requires the operator's keyboard layout — but the exposure is real
and it is named here rather than denied. Nothing is written to disk
by zylaxion itself; the dump exists only if the operator's
`core_pattern`/`RLIMIT_CORE` policy produces one.

### Finding

**LOW.** No counterexample to claim A was found: no log, no disk
write, no transmission, no sequence storage, no letter translation.
The LOW grade is the named residency class above — recent scancodes
recoverable from process memory artifacts (a core dump, or a
same-uid memory reader) and the kernel-resident aggregate counters
readable by whoever bpffs permissions allow (see Dimension 3 for
the permission modes).

### Residual

- Coredump hardening: force `RLIMIT_CORE = 0` at player start (a
  libc-only, ~10 LOC change in `src/commands/player.rs`). Deferred
  to a hardening pass — it is a posture improvement, not a
  demonstrated vulnerability, and this pass changes no player
  lifecycle behavior.
- The inter-keystroke timestamps (`ts_us`) ride the ring records
  into process memory. The model uses only the previous trigger
  timestamp for repeat attenuation. No mitigation is proposed; the
  exposure is process-local and bounded.

---

## Dimension 2 — BPF program safety (claim C)

### Surface

The eBPF object's verifier story, map lifetimes, the ring buffer
producer/consumer contract, and the atomic update path.

### Threat model

A malicious or malformed keystroke stream forces the BPF program to
leak memory, produce a torn read, or corrupt a map entry userspace
then trusts.

### Evidence

**The program, end to end** (`ebpf/src/main.rs`, 308 lines): one
`tap()` body called by both attach programs. Every map access is
bounds-checked by construction — `Array::get` returns `Option`
(index past `max_entries` yields `None`, never a fault: a keycode
above 255 is simply uncounted), `HashMap::get` is a hash lookup
that cannot overrun anything. There are no loops, no pointer
arithmetic on attacker-controlled data, and the `dev` pointer is
extracted per the ABI and never dereferenced (the passive-tap law,
`src/ebpf/args.rs:53`). Every path returns 0 — the input path is
never failed, blocked, or altered.

**Ring buffer record layout.** `events.reserve::<KeyRingEvent>(0)`
returns a `MaybeUninit` slot; `entry.write(...)` initializes all 24
bytes before `entry.submit(0)`. The aya ring buffer contract makes
a reserved record invisible to consumers until submit, so a torn
read from a partially written record is impossible by construction.
A failed reservation (buffer full — the player stalled longer than
64 KiB of keystrokes) drops the event silently: the documented
event-loss semantics, a missed click instead of a blocked input
path.

**Keymap bounds.** `keymap` is `HashMap<u32, u32>` (512 entries):
the lookup `keymap.get(&a.code)` is safe for any `u32` the kernel
hands over. The stats index rides `Array::get(a.code)` against 257
slots — bounds-checked, out-of-range uncounted.

**BPF_ATOMIC fetch-add.** The counters are `AtomicU64` updated with
`fetch_add(1, Relaxed)`. The RMW cannot fail in a way that needs
handling; the required `cpu v3` codegen is forced in
`build/ebpf.rs` (`-Ctarget-cpu=v3`) and in the committed
`ebpf/.cargo/config.toml`. Because the per-key add and the total add
are two separate RMWs, a userspace reader can observe the pair
microscopically out of sync; the only consumers are aggregate
displays, so this is benign and disclosed here. The zelynic
NIGHT-improve-29 lost-update lesson (no plain shared-field access on
the update path) is honored.

**Map lifetimes.** All four maps plus the program and link are
pinned at attach and unpinned by detach in link-first order
(`src/ebpf/pin.rs:44`); the player holds map fds on the pinned
inodes. A failed attach plants nothing but the classified failure
marker (`src/ebpf/loader.rs:534`, the fail protocol).

**Loaded program hash vs built object — INCONCLUSIVE.** The
measurement needs root and bpftool (`bpftool prog show pinned
/sys/fs/bpf/zylaxion/tap`, compare against `sha256sum` of the built
object and the attach summary's printed byte length). The sandbox
has neither. By construction the loader embeds the exact bytes
staged into `OUT_DIR` (`include_bytes!` in `src/ebpf/loader.rs:65`),
so the loaded object is the built object at the ELF level; what
cannot be measured here is the live-host confirmation. To conclude:
attach on a real host, hash the object, dump the pinned program,
compare.

### Finding

**NONE.** Every safety property is provable from the source; the
verifier re-proves the whole body at load time on a real host, and
the GPL license section keeps the load legal.

### Residual

The loaded-hash confirmation is an inconclusive measurement inside
this dimension (see above), not a vulnerability.

---

## Dimension 3 — Privilege boundaries

### Surface

What attach does with root, what the pins grant to non-root, what
the player can do, and who can signal whom.

### Threat model

Unprivileged escalation through the tap, or a root attach leaving a
persistent capability reachable by a non-root process that should
not have it.

### Evidence

**The root operations, enumerated.** `attach` is the one surface
that requires root (`src/session/privilege.rs:106`, REQUIRE gate;
polarity pinned by tests after the night-badas-1 inversion lesson).
Under root it: loads the program, creates and pins the link,
initializes and pins four maps, and — the hand-off — chowns every
pin file to `SUDO_USER` (`src/ebpf/loader.rs:568`). After attach
returns, nothing of zylaxion is resident in userspace. A non-sudo
root shell leaves the pins root-owned and says so in the summary.

**What the pins grant.** The chown transfers uid/gid only. The
invoking user's player and CLI can then open the pinned maps —
read the control plane, consume the ring buffer, read the stats
counters, write control slots (enabled, preset, volume, pid, state).
That is the architecture's design: the CLI verbs are the writers,
and bpffs file permissions are the gate.

**Can a non-root process write the control map?** A process running
as the pin-owning user can — by design, that IS the product's
control plane (the CLI's `on`/`off`/`volume` are map writes). A
different user cannot open the files if bpffs created them
non-world-writable — and that is exactly the fact this sandbox
cannot measure. The bpf() syscall layer adds a second gate
(`kernel.unprivileged_bpf_disabled`; distros with
CONFIG_BPF_UNPRIV_DEFAULT_OFF=y refuse every plain-user map op and
zylaxion classifies that refusal, `src/session/kernel.rs:133`).

**Can a non-root user run `_play`?** Yes, and it needs no
capabilities: open pinned maps (subject to the two gates above and
file ownership) and talk to the session audio server. The root
player is refused outright (`session_refusal`) — a root player
would open audio in root's session, which no user desktop shares.

**Can a non-root process SIGKILL another user's player?** No. The
kill paths are `detach`'s reap (reads the pid from the control map,
then `kill()`), and `on`'s orphan sweep (exact-match
`/proc/*/cmdline` scan for this binary + `_play`,
`src/session/orphan.rs:56`). Even if a foreign user read the pid or
matched the cmdline, `kill()` against another uid's process returns
EPERM — the kernel enforces the boundary; zylaxion does not need to.
The one deliberate exception is root: `sudo zylaxion attach`'s
orphan sweep can reap any user's exact-match player, and root can
already signal anything — the sweep's exact-match contract keeps
unrelated processes untouchable.

### Finding

**INCONCLUSIVE.** (a) What could not be measured: the actual file
modes of the pinned objects after attach on a real bpffs — the fact
that decides whether a *different* user on the same host can read
the maps (the stats histogram, the control plane) or not. The
loader never sets a mode, so the answer lives in the kernel's bpffs
defaults, which the audit will not guess. (b) Why: the sandbox has
no root, no mountable bpffs, no bpftool. (c) What would be needed:
attach on a real host as root through sudo, then
`stat /sys/fs/bpf/zylaxion/*` and a second-user access probe
(`sudo -u nobody bpftool map dump pinned /sys/fs/bpf/zylaxion/stats`).
Everything else in this dimension is verified from source and
passes; the inconclusive item gates the cross-user isolation claim,
so the dimension as a whole refuses a PASS it cannot prove.

### Residual

If the real-host stat shows world-readable pins, the follow-up is a
one-line `fchmod` in the hand-off path — pre-named here so the next
pass knows exactly where to look.

---

## Dimension 4 — Network surface (claims B and D)

### Surface

Every byte sent or received, every listener opened.

### Threat model

An attacker on the network interacts with the user without their
knowledge.

### Evidence

**Every network call site.** The sweep:

    rg -n 'TcpListener|TcpStream|UdpSocket|UnixListener|UnixStream|
           socketpair|socket\(|connect|bind\(|listen\(|std::net|
           reqwest|ureq|hyper|http' src/ --type rust

returns zero socket call sites. The std::net family never appears
in the source. `Command::new` appears exactly twice: the player
spawn (`src/commands/sound.rs:255`, self-exec, no network) and the
curl invocation in `--check-update` (`src/commands/update.rs:163`).
There is no TCP listener, no UDP listener, no Unix socket, no
socket pair, no abstract namespace socket anywhere in the tree.

**The player at runtime.** The process's only IO is the pinned ring
buffer fd, the pinned control map fds, the audio device, and
signals. The one nuance named honestly: the audio backend
(libasound against PipeWire/PulseAudio) holds client-side
connections to the user's own session audio server — that is the
sound path every desktop audio app has, it is per-session, and it
is neither a listener nor an internet endpoint. A live strace would
show exactly that; the sandbox cannot run a player (no audio
device), so the claim rests on the documented reasoning the task
accepts: zero socket call sites in the source plus the audio-server
client connection named above.

**`--check-update` in detail.** The URL is a compile-time constant
pointing at `https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest`
(`src/commands/update.rs:32`) — nothing user- or
environment-controlled selects the endpoint. TLS is curl's own
(system curl, system CA store, default verification on; the
response's HTTP status is checked). The response is parsed for one
field, `"tag_name"`, by a string search; the tag is compared
SemVer-style and printed. It is never written to disk, never
evaluated, never passed to a shell, and it cannot cause code
execution. Network failures, curl absence, non-200, and malformed
JSON are all graceful messages.

**Root refusal before network I/O.** `run_check_update` fires the
privilege guard as its first statement
(`src/commands/update.rs:156`) — under euid 0 it prints the
"do not use sudo" verdict and exits 1 before curl is ever spawned.
The `on`/`_play` surfaces refuse root the same way before any map
write or spawn. Considered and dismissed: a hostile PATH could
substitute a fake `curl` — but an attacker who controls the user's
PATH already owns the user's session, so this is not a boundary
this binary can or should defend.

### Finding

**NONE.** Claim B and claim D hold against the source.

### Residual

None in this dimension beyond the sandbox-measurement note above.

---

## Dimension 5 — Supply chain

### Surface

Every direct dependency, every transitive crate, every build-time
tool the source build requires.

### Threat model

A compromised crate lands attacker code, or a build-time tool is
fetched from an unsigned source.

### Evidence

**Direct dependencies (pinned major versions, Cargo.toml).**
arc-swap 1.7, aya 0.14 (optional, default-on via the `ebpf`
feature), clap 4 (derive), cpal 0.15, crossbeam-channel 0.5,
env_logger 0.11 (default-features off), log 0.4, nix 0.29 (signal,
poll, user), ringbuf 0.5, libc 0.2, signal-hook 0.3
(default-features off), alsa-sys 0.3.1 (Linux-gated). Twelve
direct deps; the lock resolves 150 packages total.

**The confirmed absences.** `rg '^name = "(time|serde|serde_json|
toml|chrono|jiff)"' Cargo.lock` returns nothing: no time crate, no
serde, no config parser — the claims battery's daemon-era sweep
guard these as regression tests. The `tokio` entry in the lock is
explained, not alarming: it is a dependency of
`wasm-bindgen-futures`, which rides cpal's wasm32 target stanza;
target-gated crates appear in the lock for every target but never
compile for the Linux production build (the tokio entry itself
carries only `pin-project-lite` — no runtime, no network features).

**cargo audit.** Declared part of the local battery
(`scripts/build.sh` step 4) and CI's supply-chain lane. In this
sandbox it was not installed and installing it burns minutes the
owner's rules forbid; the skip is named here and the gate runs it
in CI on every push. (If the local battery below finds it present,
it runs.)

**The ebpf-prebuilt/ lane.** The registry lane ships
`ebpf-prebuilt/zylaxion-tap` (2936 bytes) with `manifest.toml`
recording schema, generation time, toolchain
(`nightly-2026-09-18`), bpf-linker 0.11.1, the ebpf source-tree
sha256, and the object sha256. This audit re-hashed the committed
object and it matches the manifest byte for byte
(`55d63fb6...5e85`). The manifest is sufficient to reconstruct the
build (pinned toolchain, pinned linker, tree digest) — sufficient
provenance on paper. What nothing enforces: the registry-install
build path (`build/ebpf.rs`, registry lane) validates the object's
ELF structure but does not re-verify its sha256 against the
manifest at stage time. The manifest is documentation today, not a
runtime check.

**bootstrap-ebpf.sh.** Downloads the bpf-linker prebuilt binary
from `https://github.com/aya-rs/bpf-linker/releases/download/v0.11.1/...`
over HTTPS with curl/wget. It verifies: non-empty archive, extract
succeeded, top-level ELF magic (`7f454c46`), and the version string
contains 0.11.1. It does not pin a checksum or a signature for the
artifact. HTTPS authenticates the transport to GitHub, not the
artifact to a key the project controls; a compromised release asset
or a compromised mirror of the release page would pass every check
the script performs. This is the finding.

### Finding

**LOW.** The build-time toolchain download (bpf-linker) authenticates
the channel but not the artifact: no pinned checksum, no signature.
Exploitation requires a compromised aya-rs release asset or a
MITM at the transport layer below the system CA set, and the tool
runs at build time on a developer host, not on the player's runtime
path — hence LOW, and hence named rather than silently accepted.

### Residual

- Pin the bpf-linker artifact's sha256 in bootstrap-ebpf.sh (and
  ideally verify the aya-rs release signature). Deferred: it edits
  the supply-chain policy the audit is forbidden to change, and the
  honest fix belongs to its own pass.
- Enforce the ebpf-prebuilt manifest sha256 inside
  `build/ebpf.rs`'s registry lane (a ~15 LOC change, but it touches
  the build lane the freeze pass owns; named, not done).

---

## Dimension 6 — Update path and version honesty

### Surface

`--check-update`, `-V` output, the build stamp, and the release
verification flow.

### Threat model

A user is tricked into installing a malicious binary because version
output lies, or `--check-update` points at an attacker-controlled
URL.

### Evidence

**The check source.** Covered in Dimension 4: constant URL, system
curl, status checked, one field parsed, printed, never executed,
root refused before any I/O.

**`-V` output.** The version report renders from
`src/info/build.rs`: `VERSION` is `env!("CARGO_PKG_VERSION")` (the
manifest, not a hardcoded string), and `git_sha` is
`option_env!("GIT_HASH")` — stamped by `build.rs:38`, which runs
`git rev-parse --short HEAD` against the tree being compiled and
falls back to the honest literal "unknown" only when git is
unavailable. The hash is computed, not asserted. The rerun
triggers (`../.git/HEAD`, `../.git/refs`) keep the stamp in sync
with the compiled tree.

**The build line.** `ZYLAXION_BUILD` is forwarded from the
environment when a CI/release build exports it; plain builds fall
back to compile-time arch+libc detection — and the release-profile
contract is guarded by a test that fails if the manifest drifts
(`src/info/build.rs`, release-profile pin). A debug build never
claims the release contract (`profile_line()`).

**The release verification flow.** docs/VERIFY_RELEASE.md documents
three checksum files per archive (SHA-512, BLAKE2b-512, SHAKE256)
plus a GPG detached signature. `.github/workflows/release.yml`
emits exactly those (`sha512sum`, `b2sum`, the python SHAKE256
invocation at lines 309-316) and carries the GPG signing lane with
its own skip posture when no key is configured. The documented
walkthrough matches what the pipeline emits — including the
`-V`-label-matches-tarball-name smoke check.

**Stale claims found and corrected in this pass.** Three doc-honesty
items, all corrections of text, no code behavior touched:

- SECURITY.md's supported-versions table said "11.x (main)" while
  the manifest is 80.0.0-alpha.1 — the version row now matches the
  manifest.
- README's verification section described examples carrying an
  `--dump-scancodes` opt-in; no such flag exists anywhere in the
  tree (the examples only synthesize scancode 30 for audio sanity).
  The sentence now describes the tree as it is.
- `src/session/signals.rs` and `src/main.rs` module docs still
  named the retired daemon-era drop order (`LibinputSource`, the
  flock). The comments now describe the actual teardown set.
  These are the comment-reference refreshes the audit's standing
  rules demand; the historical references elsewhere in the tree
  ("died with the daemon") are accurate history and stay.

### Finding

**NONE.** The version surface tells the truth about the binary, the
check-update surface points where it says it points, and the
release flow verifies what the pipeline actually ships.

### Residual

None in this dimension.

---

## Attack-surface summary

What is reachable from where:

- **Kernel (after `sudo zylaxion attach`)**: one passive fentry or
  kprobe program on `input_event()`, four pinned maps plus a
  failure marker under `/sys/fs/bpf/zylaxion/`. Reachable by: the
  pin-owning user (map ops via bpf()), root (everything), and the
  kernel itself (the input path calls the program). Nothing listens.
- **Player (`zylaxion _play`, unprivileged)**: consumes the ring
  buffer, polls the control map, writes its pid/state, renders
  audio to the session server. Holds no socket, no file writes, no
  IPC. Signals: stoppable by its own uid (and root) only.
- **CLI (unprivileged)**: map reads/writes against the pinned
  control plane and the telemetry read; refuses root on the
  audio-session surfaces; one outbound network call behind an
  explicit flag with a root pre-refusal.
- **Network**: exactly one outbound HTTPS GET to a constant GitHub
  API URL, only on `--check-update`, never as root. Inbound: none,
  anywhere.
- **Disk**: nothing is ever written (no config, no history, no
  logs; the log stream goes to stderr only).

## TCB inventory

| Component | Trust role | What it can see | What defeats it |
|-----------|-----------|-----------------|-----------------|
| The Linux kernel (5.15+) | Runs the tap, owns the maps, enforces bpffs perms and signal boundaries | Every keystroke crossing `input_event()` | A kernel compromise — outside the model |
| The pinned objects on bpffs | The entire state store | Aggregate counters (stats), control slots, the live event stream | Whatever uid owns the pins (measured on a real host — Dimension 3), or root |
| The player process | Turns events into sound | Recent scancodes in memory (ring, channel, active voices), the previous trigger timestamp | A same-uid memory reader, or a core dump if the operator's policy produces one |
| The CLI binary | Control plane + the one network call | Whatever its invoking user can open | Replacing the binary (then it is not zylaxion) |
| build.rs + the toolchain | Produces the binary and embeds the tap object | The source tree, the git stamp | A compromised build host or an unsigned bpf-linker download (Dimension 5) |

## Posture statement

The audit examined six dimensions and produced: **2 findings
(both LOW — Dimension 1 keystroke-confidentiality residency class,
Dimension 5 unsigned build-tool download), 1 INCONCLUSIVE dimension
(privilege boundaries — pinned-file permission modes unmeasurable in
this sandbox, with the exact measurement named), and 1 inconclusive
measurement inside an otherwise-passing dimension (Dimension 2's
loaded-program hash confirmation).** All four README claims — A
zero-leakage, B no daemon/IPC/socket/inbound, C passive kernel tap,
D one explicit outbound call — verify against the source with the
residuals named above and nothing found that contradicts them.
Three stale doc claims were corrected in this pass (SECURITY.md
version row, README's nonexistent `--dump-scancodes` example lane,
the daemon-era drop-order comments). The zero-leakage posture is
intact; the freeze pass is next.
