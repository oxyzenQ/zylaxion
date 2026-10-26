<!-- SPDX-License-Identifier: GPL-3.0-only -->
<!-- Copyright (C) 2026 rezky_nightky (oxyzenQ) -->

# The Security Audit — night-private-research-2xblade-3.6X (the hardening pass)

The follow-up the 3.6 record pre-named. One question, answered with
evidence: every residual the 3.6 audit named — the coredump carrier,
the unsigned build-tool download, the pinned-file mode question — is
either hardened in this pass, re-measured, or named with the exact
lane that owns it. Nothing silently carried.

The README's four claims stay the contract:

  A. Zero-leakage: keystrokes never leave the process.
  B. No daemon, no IPC, no socket, no network inbound.
  C. The kernel tap is passive.
  D. The only outbound network call is `--check-update`.

Grades per dimension: NONE / LOW / MEDIUM / HIGH / CRITICAL /
INCONCLUSIVE. This pass was authorized to fix within 100 LOC across 2
files, hands off the DSP math, the preset registry, the CLI surface,
and the eBPF attach chain. The previous generation's full record is
the tree at commit `47f211a` (2xblade-3.6); this file supersedes it as
the current-generation record.

## Method and sandbox honesty

The security-relevant tree was re-read end to end at this pass's
tree: `ebpf/src/main.rs`, `src/ebpf/` (loader, bpf_syscall, pin,
teardown, embedded, args), `src/session/` (kernel, privilege, signals,
orphan), `src/commands/` (player, sound, attach, update, stats, info),
`src/core/`, `src/engine/`, `src/profiles/`, `build.rs`,
`build/ebpf.rs`, `src/info/build.rs`, `Cargo.toml`, `Cargo.lock`,
`scripts/dev/bootstrap-ebpf.sh`, `scripts/build.sh`,
`scripts/gate-keepers.sh`, `ebpf-prebuilt/manifest.toml`, README,
SECURITY.md, docs/VERIFY_RELEASE.md.

The sandbox in this pass: uid 1001 (non-root), no bpffs mounted, no
bpftool, no audio device, `kernel.unprivileged_bpf_disabled = 0`
(this sandbox's value, not a claim about hosts). One instrument the
3.6 pass lacked is present now: **cargo-audit 0.22.2**, installed
this pass, run with `--deny warnings` against all three lockfiles.
The evidence greps below were re-run against this tree, not quoted
from the old record.

## The fixes this pass shipped (the accounting)

3.6 named two residuals whose honest fix fits this pass's budget.
Both landed; both are live-verified in this sandbox.

- **The coredump carrier (Dimension 1's residual).** `force_no_coredump()`
  in `src/commands/player.rs`: the player now drops `RLIMIT_CORE` to
  zero — soft and hard — immediately after the logger comes up and
  before anything that can hold a scancode exists (the first
  scancode-capable object, the pinned control map, is opened later).
  Lowering the hard limit too means the process itself cannot re-raise
  the ceiling without `CAP_SYS_RESOURCE`. Best-effort by contract: a
  failure is logged, never fatal. Verified by two new test pins
  (`test/commands/player_tests.rs`): the limit reads 0/0 after the
  call from any legal starting point, and the call is idempotent.
  ~10 LOC of mechanism, ~50 LOC with the contract docs and the call
  site, per the residual's own estimate.
- **The unsigned bpf-linker download (Dimension 5's LOW).**
  `scripts/dev/bootstrap-ebpf.sh` now carries the measured sha256 of
  both musl release assets (x86_64, aarch64) beside the version
  constant, and the installer dies at the checksum gate — naming both
  the measured and the pinned hash — before extraction, before the
  ELF check, before anything on the host executes bytes it did not
  build. Verified live in this sandbox on both paths: a tampered pin
  (one flipped nibble) is refused with both hashes printed and
  nothing installed or executed; the honest archive passes the gate
  and installs. +27 lines.

**The budget, transparently:** the fix is 2 files, 77 added lines —
under the 100-line cap. The proof harness (`test/commands/
player_tests.rs`, 63 lines) is a third file because the layout law
(check-loc.sh) requires every test to live in the test/ tree, never
inline in src/ — the repo's own standing rule makes one-file-more the
honest shape, and the alternative (shipping the fix untested) violates
a harder rule.

The two residuals this pass does NOT touch, with owners:

- The ebpf-prebuilt manifest sha256 is still documentation, not a
  runtime check — enforcing it inside `build/ebpf.rs`'s registry lane
  is a ~15 LOC change to the build lane the freeze pass owns (3.6's
  wording, unchanged).
  Status update (night-improve-31, 2026-10-15): the enforcement seat
  landed, one lane earlier than this row asked for — the commit-time
  prebuilt gate (scripts/gates/check-prebuilt-parity.sh wired through
  .githooks/pre-commit) proves the manifest against the live `ebpf/`
  tree on every commit, and the ship-time parity steps keep their
  seat. The build.rs-internal row this bullet pre-named stays with
  the freeze pass.
  Status update (night-dragon-maintenance-0, 2026-10-26): the digest
  algorithm is blake3 (manifest keys ebpf_tree_blake3/blake3, schema
  2, the reference b3sum binary), and the registry lane itself now
  verifies the staged object's blake3 against the manifest pin before
  embedding (build/prebuilt.rs) — the row's original ask is fully
  retired.
- The pinned-file permission modes stay INCONCLUSIVE (Dimension 3) —
  no bpffs host here to measure; the fchmod follow-up stays
  pre-named in the same place 3.6 left it.

---

## Dimension 1 — Keystroke confidentiality (claim A)

### Surface

The path from the eBPF tap to the DSP engine to the audio buffer:
every struct that carries a scancode-class field, every map, every
log line, the panic handlers, and the buffers a core dump would
carry — the one buffer class this pass changed.

### Threat model

An attacker with read access to process memory, the log stream, the
syscall trace, or a core dump reconstructs typed text from the
engine's outputs.

### Evidence

**The dataflow is unchanged** from 3.6's end-to-end read: tap →
24-byte `KeyRingEvent` → ring buffer → bridge cast → `KeyEvent` →
scancode used exactly three ways (pan position, trigger identity,
release match), never rendered as a letter, never translated. The
kernel-resident state is still aggregate counters only (256 keycode
slots plus a total, `BPF_ATOMIC` fetch-add).

**The log audit, re-run at this tree:**

    rg -n 'scancode|keycode' src/ --type rust | rg 'log::|println!|eprintln!'

returns exactly one hit: `src/commands/stats.rs:74`, the disclosed
`status --verbose` aggregate render. No new log line names a keycode;
the player's new `force_no_coredump` warn path prints an errno string
and a static message — no event field, no key identity.

**The core-dump posture — changed this pass.** 3.6's honest answer
was that a core dump is a recovery carrier for a bounded recent
window of scancodes (ring pages, the event channel, the active
voices). The player now closes the file-backed dump path itself:
`RLIMIT_CORE` is forced to 0/0 at player start, before any
scancode-capable buffer is opened, and the hard limit is lowered with
the soft one so the ceiling cannot be quietly re-raised later. The
two new pins hold the 0/0 fact and the idempotence. The honest scope,
per core(5): a **piped** `core_pattern` spawns its helper regardless
of the limit — that lane is the operator's distro policy
(systemd-coredump, apport), outside any in-process lever, and it is
named here rather than claimed away.

### Finding

**LOW, narrowed.** No new counterexample to claim A. The 3.6 LOW's
core-dump carrier is closed at the strongest in-process lever; what
remains of the named residency class is the bounded recent window in
process memory against a **same-uid** memory reader (an attacker who
already owns the user's session) and the operator-piped dump lane
(the operator's own policy). Neither is reachable by the threat
model's outsider; both are named.

### Residual

- The piped `core_pattern` lane: the kernel spawns the helper
  regardless of `RLIMIT_CORE`; only the operator's coredump policy
  governs it. Documented posture, not a code gap.
- The inter-keystroke `ts_us` timestamps still ride the ring records
  into process memory. Unchanged from 3.6; the exposure is
  process-local and bounded; no mitigation proposed.

---

## Dimension 2 — BPF program safety (claim C)

### Surface

The eBPF object's verifier story, map lifetimes, the ring buffer
producer/consumer contract, the atomic update path. Unchanged by this
pass — no byte of `ebpf/` moved (the manifest's tree pin —
`ebpf_tree_sha256` at the time of this pass, `ebpf_tree_blake3` since
night-dragon-maintenance-0 —
still matches the tree's own digest discipline, and the prebuilt
object hash re-verified: `55d63fb6…9ea03`, byte for byte).

### Threat model

A malicious or malformed keystroke stream forces the BPF program to
leak memory, produce a torn read, or corrupt a map entry userspace
then trusts.

### Evidence

Every 3.6 property re-verified from the same source, unchanged: every
map access bounds-checked by construction (`Array::get` returns
`Option`; a keycode above 255 is uncounted, never a fault), no loops,
no attacker-controlled pointer arithmetic, the `dev` pointer
extracted per ABI and never dereferenced (the passive-tap law), every
path returns 0. The ring record is fully initialized before submit —
a torn read is impossible by construction; a failed reservation drops
the event (documented loss semantics) instead of blocking input.
`BPF_ATOMIC` fetch-add on `AtomicU64` counters, `cpu v3` codegen
forced in `build/ebpf.rs`. Attach pins/unpins in link-first order.

**Loaded program hash vs built object — still INCONCLUSIVE.** The
measurement needs root and bpftool; this sandbox has neither, same as
3.6. By construction the loader embeds the exact bytes staged into
`OUT_DIR` (`include_bytes!`), so the loaded object is the built
object at the ELF level; the live-host confirmation stays named:
attach on a real host, hash the object, dump the pinned program,
compare.

### Finding

**NONE.** Every safety property is provable from the source; the
verifier re-proves the whole body at load time on a real host.

### Residual

The loaded-hash confirmation remains the one inconclusive measurement
inside this dimension — a measurement gap, not a vulnerability.

---

## Dimension 3 — Privilege boundaries

### Surface

What attach does with root, what the pins grant to non-root, what the
player can do, who can signal whom.

### Threat model

Unprivileged escalation through the tap, or a root attach leaving a
persistent capability reachable by a non-root process that should not
have it.

### Evidence

**The source-level facts hold, re-confirmed at this tree.** `attach`
is the one root-requiring surface (REQUIRE gate, polarity
test-pinned); under root it loads, pins, and chowns every pin file to
the desktop user — resolved from `SUDO_USER` (the interactive sudo
lane) first, then from the first `/run/user/<uid>` desktop runtime
(the boot lane, night-dinner-19: the boot oneshot runs before anyone
logs in, and the hand-over units deliver the pins at the first
login) — after attach returns, nothing of zylaxion is resident
in userspace. The chown transfers uid/gid only; the desktop user's
player and CLI open the pinned maps, which IS the control plane by
design. The kernel's own gates stack on top:
`kernel.unprivileged_bpf_disabled` (classified in
`src/session/kernel.rs`) and `kill()`'s cross-uid EPERM (the orphan
sweep's exact-match contract cannot touch unrelated processes even
under the root sweep's wider reach). The root player is refused
before any map write or spawn; the root update-check is refused
before any network I/O.

**What this sandbox still cannot measure — unchanged from 3.6:** the
actual file modes bpffs creates for the pinned objects after a real
attach. The loader never sets a mode, so the cross-user answer lives
in kernel bpffs defaults, and this pass will not guess. The sandbox
has no root, no mountable bpffs, no bpftool.

### Finding

**INCONCLUSIVE** — carried from 3.6 with the same named measurement:
attach on a real host as root through sudo, then
`stat /sys/fs/bpf/zylaxion/*` and a second-user access probe
(`sudo -u nobody bpftool map dump pinned /sys/fs/bpf/zylaxion/stats`).
Everything else in the dimension verifies from source and passes; the
inconclusive item gates the cross-user isolation claim, so the
dimension as a whole refuses a PASS it cannot prove.

### Residual

If the real-host stat shows world-readable pins, the follow-up is the
one-line `fchmod` in the hand-off path — still pre-named, still owned
by the pass that can attach. This pass does not guess a fix it
cannot measure.

### Addendum — night-total-lts-1 (2026-10-15): the map-pid kill path, closed

The dimension's evidence above cited the orphan sweep's exact-match
contract as the reason `kill()` cannot touch unrelated processes.
That contract covers the attach/`on` sweep only; the detach-side
reaper (night-improve-27, landed after this pass's tree) read the
pid from the control map instead — a map the pin-owning user can
write — and drove the SIGTERM/SIGKILL ladder at it with no identity
check: a planted `PLAYER_PID` slot aimed a root-mediated kill at an
arbitrary process the moment an admin ran `sudo zylaxion detach`.
Closed in night-total-lts-1: the reaper now reads the candidate's
`/proc/<pid>/cmdline` and requires the exact player shape through
the same pure matcher the sweep trusts (`orphan::pid_is_our_player`,
reusing `cmdline_matches_player`), and refuses to signal anything
else — the cleared enabled gate stays the real player's stop signal,
so the fix costs the signals only for a wedged process that is not
provably zylaxion's. Pinned by
`the_map_pid_gate_admits_only_the_player_shape` in the orphan test
tree (the honest shape, foreign cmdlines, a different-install
player, near misses, and all three unverifiable shapes refuse); the
live stub-reap leg proves the honest path unregressed. Residual,
named honestly: the validate-then-kill window is a pid-recycling
race measured in microseconds — the same order the unvalidated code
had, inherent to any kill-by-pid design — and the gate's refusal
direction (skip the kill, keep the unpin) is the safe side of it.

---

## Dimension 4 — Network surface (claims B and D)

### Surface

Every byte sent or received, every listener opened.

### Threat model

An attacker on the network interacts with the user without their
knowledge.

### Evidence

**The sweep, re-run at this tree** (pattern: the std::net family,
TcpListener/TcpStream/UdpSocket/UnixListener/UnixStream,
socketpair/socket/connect/bind/listen, reqwest/ureq/hyper/http):
zero socket call sites in `src/`. The only phrase-level hits are doc
comments and the channel `Disconnected` variants — no syscall.
`Command::new` appears exactly twice, same as 3.6: the player spawn
(`src/commands/sound.rs:255`, self-exec, no network) and the curl
invocation in `--check-update` (`src/commands/update.rs:163`).

**`--check-update` unchanged:** constant compile-time URL
(`https://api.github.com/repos/oxyzenQ/zylaxion/releases/latest`),
system curl, system CA store, default TLS verification on, HTTP
status checked, one field (`"tag_name"`) parsed by string search,
compared SemVer-style, printed — never written to disk, never
evaluated, never executed. Root is refused as the first statement,
before curl is ever spawned.

**The player's runtime IO is unchanged** — and the new hardening adds
no IO: `force_no_coredump()` is one setrlimit call, no fd, no
network. The audio backend's per-session client connection to the
user's own audio server remains the only external connection class,
neither a listener nor an internet endpoint.

### Finding

**NONE.** Claims B and D hold against this tree.

### Residual

None beyond the standing sandbox-measurement note (a live strace of a
playing player needs an audio device this sandbox lacks; the claim
rests on zero socket call sites plus the audio-server client class).

---

## Dimension 5 — Supply chain

### Surface

Every direct dependency, every transitive crate, every build-time
tool the source build requires — and the one tool download this pass
changed the posture of.

### Threat model

A compromised crate lands attacker code, or a build-time tool is
fetched from an unsigned source.

### Evidence

**cargo-audit ran this pass** — the instrument 3.6 had to skip.
`cargo audit --deny warnings` exits 0 against all three lockfiles:
the workspace lock (150 packages), the ebpf crate lock (31), the
audit harness lock (119), against 1314 RustSec advisories. Zero
vulnerabilities, zero warnings.

**The lockfile absences, re-verified:** `time`, `serde`, `serde_json`,
`toml`, `chrono`, `jiff` — none present in the workspace lock. The
`tokio` entry remains the explained wasm32-gated cpal transitive that
never compiles for the Linux production target.

**bootstrap-ebpf.sh — changed this pass, verified live.** The script
now pins the sha256 of both musl release assets beside the version
constant, measured over HTTPS from the aya-rs release page and
committed in-tree (x86_64 `e058a6ae…16e8`, aarch64
`341ec1c5…f1380`). The checksum gate fires before extraction and
before execution. Both paths proven in this sandbox: a tampered pin
is refused with both hashes named, nothing installed or executed;
the honest archive passes and installs. The epistemics, stated
plainly: the pin's root of trust is the repo's own commit process —
the same epistemic class as Cargo.lock itself. It converts "no
checksum at all" into "a checksum the repo owns"; it cannot defend
against a compromise at measurement time, and it does not replace a
signature (named below).

**The ebpf-prebuilt lane, re-verified:** the committed object hash
matches `manifest.toml` byte for byte (`55d63fb6…9ea03`). The
manifest remains provenance on paper; the runtime enforcement point
is `build/ebpf.rs`'s registry lane and stays the freeze pass's.

### Finding

**NONE.** The 3.6 LOW (the unauthenticated bpf-linker artifact) is
fixed this pass and the fix is live-verified on both the accept and
reject paths. No new counterexample in the dimension.

### Residual

- Signature verification of the aya-rs release asset (beyond the
  checksum pin) — the next posture step if the owner wants one;
  named, not done.
- The manifest-sha256 enforcement inside `build/ebpf.rs`'s registry
  lane — owned by the freeze pass, unchanged from 3.6.
  Status update (night-dragon-maintenance-0, 2026-10-26): landed —
  build/prebuilt.rs verifies the staged object's blake3 against the
  manifest pin before embedding; the digest algorithm is blake3
  end to end.

---

## Dimension 6 — Update path and version honesty

### Surface

`--check-update`, `-V` output, the build stamp, the release
verification flow.

### Threat model

A user is tricked into installing a malicious binary because version
output lies, or `--check-update` points at an attacker-controlled
URL.

### Evidence

**The check source** is unchanged (Dimension 4's detail holds).
**The version surface, re-measured in this sandbox:** the debug
binary's build line reads `linux-amd64-gnu (47f211a)` and
`git rev-parse --short HEAD` in the same tree returns `47f211a` —
the stamp is computed from the tree being compiled
(`build.rs:38`, rerun-triggers on `.git/HEAD` and `.git/refs`), the
version rides `CARGO_PKG_VERSION` (the manifest), and a build
without git falls back to the honest "unknown".

**The release flow is unchanged:** three checksum files per archive
(SHA-512, BLAKE3-256, SHAKE256) plus the GPG detached signature,
emitted by `.github/workflows/release.yml` exactly as
docs/VERIFY_RELEASE.md documents, including the
`-V`-label-matches-tarball-name smoke check. No stale claims were
found in this pass — 3.6's three corrections (SECURITY.md version
row, README's examples lane, the drop-order comments) hold at this
tree.

### Finding

**NONE.** The version surface tells the truth about the binary, the
check-update surface points where it says it points, the release
flow verifies what the pipeline ships.

### Residual

None in this dimension.

---

## Attack-surface summary (delta from 3.6)

- **Kernel (after `sudo zylaxion attach`)**: one passive fentry or
  kprobe program on `input_event()`, four pinned maps plus a failure
  marker under `/sys/fs/bpf/zylaxion/`. Reachable by the pin-owning
  user, root, and the kernel itself. Nothing listens. Unchanged.
- **Player (`zylaxion _play`, unprivileged)**: unchanged in shape —
  ring buffer in, audio out, control-map writes, no socket, no file
  writes — with one new property: **`RLIMIT_CORE` is pinned to 0/0
  for the process's whole life**. A file-backed core dump of the
  player is no longer producible by the player itself.
- **CLI (unprivileged)**: unchanged; one outbound network call behind
  an explicit flag with a root pre-refusal.
- **Network**: exactly one outbound HTTPS GET to a constant GitHub
  API URL, only on `--check-update`, never as root. Inbound: none,
  anywhere. Unchanged.
- **Disk**: nothing is ever written by the tool. Unchanged.
- **Build toolchain**: the bpf-linker download is now authenticated
  against a committed sha256 pin, refused before execution on
  mismatch. Changed this pass.

## TCB inventory (delta rows)

| Component | Trust role | What it can see | What defeats it |
|-----------|-----------|-----------------|-----------------|
| The Linux kernel (5.15+) | Runs the tap, owns the maps, enforces bpffs perms and signal boundaries | Every keystroke crossing `input_event()` | A kernel compromise — outside the model |
| The pinned objects on bpffs | The entire state store | Aggregate counters, control slots, the live event stream | Whatever uid owns the pins (the Dimension 3 measurement on a real host), or root |
| The player process | Turns events into sound; runs with RLIMIT_CORE 0/0 | Recent scancodes in memory (ring, channel, active voices), the previous trigger timestamp | A same-uid memory reader; an operator-piped core dump (the distro's policy, not the player's) |
| The CLI binary | Control plane + the one network call | Whatever its invoking user can open | Replacing the binary (then it is not zylaxion) |
| build.rs + the toolchain | Produces the binary and embeds the tap object | The source tree, the git stamp | A compromised build host; the bpf-linker artifact is checksum-pinned (Dimension 5) — a tampered pin or a measurement-time compromise is outside the model |

## Posture statement

The hardening pass examined six dimensions against the tree with the
3.6 residuals as its worklist: **no new findings; the two pre-named
fixes shipped (77 LOC across the two authorized files, plus the
layout-law test harness) and are live-verified in this sandbox; 1
INCONCLUSIVE dimension remains (privilege boundaries — pinned-file
permission modes unmeasurable without a real bpffs host, with the
exact measurement named for the pass that can attach), and 1
inconclusive measurement stays inside Dimension 2 (the loaded-program
hash confirmation, root+bpftool required).** Dimension 1 narrows from
LOW to LOW-narrowed (the file-backed coredump carrier is closed;
same-uid memory reads and operator-piped dump policy are the named
remainder); Dimension 5 closes its LOW with the checksum pin and
carries the freeze-pass residual forward by name. All four README
claims — A zero-leakage, B no daemon/IPC/socket/inbound, C passive
kernel tap, D one explicit outbound call — verify against this tree.
The zero-leakage posture is intact and now enforced one lever
deeper; the freeze pass is next, and its residual is waiting,
pre-named, in build/ebpf.rs.

## Addendum — night-dinner-3: the bpffs front door relax

A privilege-boundary change shipped with the Debian 13 fix
(docs/KERNEL_COMPATIBILITY.md carries the full evidence chain), and
the security ledger must name it like any other lever.

**What changed.** `sudo zylaxion attach` may now add the
other-execute bit to the bpffs mount root (`chmod o+x /sys/fs/bpf`)
— and ONLY that bit — when, and only when, both hold: the pins were
successfully handed to the invoking sudo user, and that user's walk
to the pins is blocked by the mount root's mode (the Debian 13
`mode=700` shape). The relax rides the existing hand-over; it is not
a standalone lane, not a flag, and it never fires for a plain root
shell.

**What the bit grants.** A non-root user who already knows a path
under `/sys/fs/bpf` may now resolve that path — stat() through the
mount root. Nothing more: without `o+r` there is no listing (the
mount root's directory contents stay invisible), and every object
inside keeps its own file mode — zylaxion's pins stay `0600`
owner-only (they are chowned to the invoking user the moment attach
plants them), and every other program's kernel objects keep whatever
their creator chose.

**What the bit does not grant.** No read on the mount root, no
write anywhere, no new capability on any pin, no access to maps
whose owner did not hand them over. The kernel's unprivileged-bpf
gate (the `kernel.unprivileged_bpf_disabled` posture the doctor
reports) still decides whether a plain user may issue bpf() syscalls
at all — the walk bit moves the front door, not the gate.

**The residual, named.** Other bpffs consumers that deliberately
pinned world-readable objects (modes with an `o+r`) behind a 0700
mount relying on the shut door gain unintended readability for
walkers who know their paths. That is a system-administrator
decision surface (the undo is one command — `sudo chmod o-x
/sys/fs/bpf` — and the doctor names it), and on the distro whose
default forced this fix the objects under bpffs are root-placed by
systemd tooling that does not pin world-readable state. The
trade-off is recorded rather than hidden: one traversal bit against
the product's core promise ("sudo attach once, then every verb is
unprivileged"), which a 0700 mount silently broke.

## Addendum — night-dinner-13: the host measurement gets its instrument

The one INCONCLUSIVE dimension this ledger carried (Dimension 3 — the
pinned-file permission modes, unmeasurable without a real bpffs host)
and the one inconclusive row inside Dimension 2 (the loaded-program
identity, root + bpftool required) now have their measuring stick:
`scripts/audit/zactrix-host.sh`.

The script is read-only by default (it measures whatever posture the
host is already in) and carries a `--full` lane for the mutating
lifecycle row (off -> detach -> attach -> on -> soak -> off, with the
recorded preset/volume/player state restored). Its rows:

- R1+R2+R3 — Dimension 3's named measurement: the pin census, the
  per-pin uid:gid mode matrix on the real bpffs, the mechanical
  verification of the 0600 owner-only claim (including which posture
  the host is in: the unit's root lane or the session hand-off lane),
  the unprivileged walk + stat + control-map dump as the plain user,
  and the mount-root mode (the o+x relax lever above).
- R4 — Dimension 2's host facts: `bpftool prog show` of the loaded
  program (id/tag/verifier instructions, recorded for the ledger) and
  the on-disk prebuilt object against the manifest sha256.
- R5-R9 — the operational rows a sandbox structurally cannot produce:
  the boot posture (unit enabled, this boot's attach SUCCESS, journal
  clean of the Unknown-key class), kernel-log verifier hygiene, claim
  B live (no socket while the player runs), the real-input telemetry
  delta, and the player on the real audio backend (whose audible
  verdict is HUMAN on purpose).

Until the host run's output lands in this ledger, Dimension 3 stays
INCONCLUSIVE — the script does not flip the row; the measurement does.

## night-dinner-14 addendum — the host measurement landed, the instrument grew teeth

The first host paste (nightpc, kernel 6.18.50-3-cachyos-lts, tree commit
363fc85, invoking user rezky uid 1000) closed what this ledger left open.

**Dimension 3 (privilege boundaries): INCONCLUSIVE -> measured.** The
paste IS the named measurement, verbatim from the live bpffs host:

- The pin matrix: all 6 pins present under `/sys/fs/bpf/zylaxion`
  (control, events, keymap, stats, tap, tap_link), every one mode 0600,
  owned by uid 1000 — the documented session hand-off posture (attach
  chowned them to the invoking sudo user). The directory itself is
  root-owned 0755.
- The unprivileged lanes hold on the real host: the plain user lists
  the pin directory, stats the control pin, and `bpftool map dump
  pinned /sys/fs/bpf/zylaxion/control` works without privileges —
  the introspection promise is measured, not argued.
- The mount root: `/sys/fs/bpf` at mode 1777 (o+x present — either the
  distro default or the night-dinner-3 ride-along relax; the walk
  verdict above is the load-bearing row).

**Dimension 2's inconclusive row gets its host facts.** The loaded
program answered `bpftool prog show` (tracing, `fentry_input_ev`, tag
`8d3b890c802e1674`, xlated 752B), and the on-disk prebuilt object
matched the manifest sha256 — what the host loads is what the tree
certifies. The byte-exact loaded-vs-built hash stays the named
residual, unchanged.

**What the same paste surfaced (named, not buried).** The telemetry
rows (R8, R10's soak) found the press counter static across two
dedicated typing windows — "planted but deaf" — and the `--full`
lifecycle scored seventeen PASS while carrying an internally
impossible state: the pinned program's `loaded_at` predated the run's
own detach/re-attach claims, the recorded player pid outlived the
off/detach/reap sequence, and the attach had taken its idempotent
early-return. The instrument had trusted exit codes where it needed
effects. night-dinner-14 corrects the instrument (script-only; zero
engine bytes, the freeze untouched):

- Every mutating row now verifies its effect: off must retire the
  registered pid, detach must leave a kernel that answers no
  `bpftool prog show pinned` query, attach must replant a NEW program
  id (ids are monotonic — an unchanged id is the no-op, and the soak
  is skipped rather than measured against a stale generation), on
  must register a fresh pid.
- R8's deaf branch discriminates the two failure shapes via the
  program's own `run_cnt` (it counts every `input_event()` invocation
  system-wide): fired-but-gates-dropped (the enabled/keymap gates)
  versus link-dead (the kernel never calls the attach point), with a
  keymap census riding the same branch and a bpftrace one-liner as
  the named unlock.
- R5 reads journal-native success markers (the old grep wanted
  systemctl's own rendering, which journalctl never prints) and asks
  the parse-hygiene question directly: sha256 of the installed unit
  against the tree's asset, with the journal archaeology scoped to
  parses after the installed file landed.
- R0's bpffs stamp matches the fstype (`bpf`/`bpffs`) at the mount
  point instead of grepping for a word; R9's resource summary awk had
  an unterminated string that rendered empty parens.

**The open rows the next host run decides.** (a) The deaf tap: R8's
next run names the layer — gates or kernel path. (b) The boot
posture: R5's unit-file parity names whether the installed unit is
the tree's. (c) The loaded-vs-built byte hash stays named. The
measurement runs; the ledger records what it says.

## night-dinner-21 addendum — the ledger's first fully green run

The fourth host paste (nightpc, kernel 6.18.55-2-cachyos-lts, tree
commit `5eac83c`, invoking user rezky uid 1000, full sudo) is the
milestone this ledger has been converging on since the first 17-FAIL
run: **PASS 23 / FAIL 0 / INCONCLUSIVE 0** — the `--full` battery's
first run with every row measured and none inconclusive.

**R5, the boot lane, reads systemd's own verdict.** The
night-dinner-21 rewrite of the zero-marker branch is in action: the
three rows all PASS with the new honest wording — "systemd records
Result=success (ExecMainStatus 0) — systemd's own verdict is the
evidence" — and the journal text rides along as corroboration, never
as the judge. The empty-journal INCONCLUSIVE branch that produced the
first R5 verdict in this series is gone: a `systemctl show` state any
user can read decides first, the parse-hygiene row runs unconditionally
(PASS — the installed unit byte-identical to the tree's asset), and
the FAIL channel keeps the scoped-tail archaeology as depth, not as a
gate. The hand-over lane this boot is the night-dinner-19/20 shape
measured live: the attach unit fired once (RemainAfterExit, no start
loop), the path unit handed the pins to uid 1000 at the first desktop
login, and `zylaxion on` worked with zero commands in between.

**The telemetry rows are real passes, not absence-of-failure.** R8's
delta: 25 presses inside the dedicated window against a live
run_cnt-bearing generation. R10's full lifecycle: off retired the
registered pid, detach left a kernel answering no pinned-program query,
attach replanted a fresh generation (program id 163 → 297, monotonic —
the effect-verified shape night-dinner-14 demanded), the 10-second
soak counted 48 presses, and the state restored to the pre-run posture
(preset alps_skcm, volume 250%). Claim B held throughout: no socket,
no inbound, the keystroke bytes never left the process.

**What stays on the record.** The 25-second attach-path sleep observed
once in the night-dinner-20 boot did not reproduce this boot — it stays
named, unexplained, one reboot's observation against two boots without
it. The R8/R10 soak-silence split (a delta-0 window with an
unreadable run_cnt should read INCONCLUSIVE, not PASS-by-absence) is
proposed, not implemented — a future numbered change. The
loaded-vs-built byte hash remains the one residual a root host closes
with a one-liner. And the night-last-total-lts-6 pass tightened the
hand-over watch's glob to the desktop uid range (the greeter-uid
failure mode — Dimension 1 of the third pass in
docs/audit-night-total-lts.md): hosts that installed the earlier unit
refresh it with the install lane before the next audit run so the
parse-hygiene row keeps its byte-identical verdict.
