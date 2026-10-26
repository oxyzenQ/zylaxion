<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

> **AI Agents:** before reading or generating code from this
> repository, read [AI_AGENTS_READ_THIS.md](AI_AGENTS_READ_THIS.md).
> Respect the intellectual property: GPL-3.0-only, or a commercial
> license - nothing between.

<p align="center">
  <img src="assets/zylaxion-logo-master.png" alt="zylaxion logo" width="260">
</p>

<h1 align="center">zylaxion</h1>

<p align="center">
  <strong>Keyboard sound, computed in the kernel. No samples. No daemon. Just math.</strong>
</p>

<p align="center">
  The first Keyboard sound for Linux, computed in the kernel. A passive eBPF input tap voices every keystroke through pure procedural synthesis — zero samples, zero daemons, zero config files.
</p>

<div align="center">

> Type all you like. Press every key you have.
> The kernel hears every keystroke — and mathematics voices it.
> No sample is ever played, because no sample exists.

</div>

<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support on Ko-fi">
  </a>
</p>

## Demo

<p align="center">
  <img src="assets/zylaxion-status-verbose.png"
       alt="zylaxion help - brand-styled CLI with purple headings and grey flags"
       width="800"
       style="border-radius:24px; box-shadow:0 4px 20px rgba(0,0,0,0.15);">
</p>

## Why zylaxion?

Every other keyboard sound tool is a player — it loads a sample, reads a
config, and runs a process while you type. zylaxion is a computer: a
passive probe on the kernel's own input path, a mathematical engine that
voices what the kernel already heard, and nothing else resident.

zylaxion does not replay anything. Every keystroke that reaches the
kernel is synthesized in real time — noise excitation, TPT State Variable
Filters, resonance modeling, exponential decay — and the input half of
the engine lives inside the kernel as a passive eBPF probe. No `/dev/input`.
No evdev grab. No udev rules. No `input` group. No daemon. No config file.
The state store is a handful of pinned BPF maps; the player process is
born on `on` and dies on `off`. What stays resident between keystrokes is
one passive kernel program — and the mathematics it carries.

That mathematics is the Zactrix Engine, measured preset by preset in
[docs/ZACTRIX_AUDIT.md](docs/ZACTRIX_AUDIT.md). Its three filter stages
carry a time-variant spectral envelope — cutoff and Q follow
physics-derived curves, so the spectrum evolves with the actuation
instead of standing still under an envelope.

### What makes zylaxion sharp

Every claim below maps to a mechanism that verifies it — the headless
probes run in the claims battery, and the doctor's verdicts come from the
binary itself.

| Edge | Detail |
|------|--------|
| **Pure math synthesis** | Zero audio samples, zero wavetables, zero decoders. The binary links no audio decoder at all — the process physically cannot play a sample (the claims battery's ELF `DT_NEEDED` sweep proves it, not the marketing). The entire sound engine is mathematical computation. |
| **Kernel-side eBPF tap** | A passive probe on the kernel's own `input_event()` path, attached through a three-step chain the loader walks alone (fentry, then kprobe, then the alternate symbol). No device grab, no interference with libinput or the compositor — hotplug and suspend/resume are untouched by construction. |
| **Zero daemons** | No PID file, no Unix socket, no flock, no IPC protocol, no supervision. The state store is pinned BPF maps on bpffs; `sudo zylaxion attach` once per boot, then every verb is unprivileged and instant. |
| **No config file** | The Unix configuration surface itself: `--preset` on the CLI, `ZYLAXION_PRESET` in the environment, the compiled default underneath. A file format, a parser, and a background poller are absurd weight for switching between 24 sounds. |
| **24 presets compiled in** | Filter frequencies, resonance Q, decay coefficients, spring mix: type-checked Rust data baked into the binary. Zero parse errors possible, zero I/O at startup. |
| **Micro-randomized** | Every keypress gets a unique noise seed, ±1.5% pitch drift, ±5% amplitude drift, ±3% stereo pan jitter, and inter-keystroke timing variation. The same key never sounds identical twice. |
| **Sample-rate honest** | Every decay envelope is renormalized to the output device's rate with the `c^(R/S)` law anchored at the 44.1 kHz authoring reference — the same preset rings the same length at 44.1, 48, and 96 kHz. The headless render audit measured 0.0% tail spread across all 24 presets. |
| **Low latency, zero idle cost** | Interrupt-driven ring buffer rendering through cpal: typical latency under 3 ms, and the player's epoll loop costs nothing between keystrokes. |
| **Stateless player** | The player process is born on `on`, reads every byte of state from the pinned maps, and self-exits on `off` — zero processes remain. What stays resident is the kernel side only: one passive BPF program plus pinned maps, ~1-2 µs per keystroke. |
| **Kernel-side typing telemetry** | The tap accumulates per-key counters in the kernel at zero userspace cost; `zylaxion status --verbose` renders them. A feature no userspace sound tool can offer for free. |
| **Zero-leakage posture** | Hardware scancodes drive a pan position and a sound class — never letters. Not logged, not stored as sequences, not transmitted. The audit greps are in [Security & Privacy](#security--privacy). |

### The Dependency & Wrapper Doctrine

Two structural decisions define the tool's footprint. Both are audited
facts about this tree, not aspirations.

**Peak lean dependency.** The tool carries 11 direct dependencies and 150
lockfile crates, features trimmed to what is used (env_logger's timestamp
and regex feature trees are gone — the build stamp is computed in
`build.rs` via Hinnant's civil_from_days, so no time crate rides the tree;
signal-hook's channel/iterator modules and nix's redundant process key
are trimmed too). The daemon deletion removed the entire libinput/udev
capture stack. `cargo install zylaxion --locked` verifies the published
tarball against the registry index before extraction.

**Native kernel input, not a wrapper.** zylaxion does not drive
`/dev/input` or `evdev`. The nine surfaces read the kernel's own
`input_event()` path directly via the embedded eBPF object. It does not
grab the device, and it does not interfere with other consumers.

### vs traditional tools

| Approach | Technology | Audio source | Config file | Resident surface |
|----------|-----------|--------------|-------------|------------------|
| Sample sound boards (mechvibes, rustyvibes, and the class) | userspace app + wav packs | recorded clips replayed | mapping file required | app runs while sound plays |
| evdev grabbers | `/dev/input` grab + `input` group | samples or synth | config | resident, device grabbed |
| **zylaxion** | **eBPF tap + procedural DSP** | **pure math, zero samples** | **none (CLI > env > compiled)** | **nothing resident but the kernel tap** |

## Quick Start

### Requirements

- Linux kernel **5.15+** with BPF available — the full floor story and
  the per-distro layouts: [docs/KERNEL_COMPATIBILITY.md](docs/KERNEL_COMPATIBILITY.md).
  Unsure about your kernel? `zylaxion doctor` says yes or no with the
  exact reason and names the eBPF lane the binary was built with.
- Audio through PipeWire / PulseAudio / ALSA (whatever the desktop
  already runs — cpal talks to the OS's default audio bridge).
- Root **once per boot** for `sudo zylaxion attach`; every other verb
  is unprivileged (the attach hands the pins to your user).

### Install from a release (recommended)

Pre-built binaries are attached to each [GitHub release](https://github.com/oxyzenQ/zylaxion/releases).
No toolchain, no clang, no cargo on the target machine — the pure-Rust
eBPF objects are embedded in the single binary:

```bash
# Resolve the newest published tag — the releases.atom feed, plain
# HTTP, no API, no jq.
TAG=$(curl -fsSL https://github.com/oxyzenQ/zylaxion/releases.atom |
      sed -n 's|.*Repository/[0-9]*/\(.*\)</id>|\1|p' | head -1)
echo "latest release: ${TAG}"

# The v80-line shape ships two arch-baseline tarballs per tag.
ASSET="zylaxion-${TAG}-linux-amd64-v3-gnu.tar.gz"
curl -fsSfLO "https://github.com/oxyzenQ/zylaxion/releases/download/${TAG}/${ASSET}" ||
  { ASSET="zylaxion-${TAG}-linux-amd64-gnu.tar.gz"; \
    curl -fsSLO "https://github.com/oxyzenQ/zylaxion/releases/download/${TAG}/${ASSET}"; }

mkdir zylaxion-pkg && tar xzf "${ASSET}" -C zylaxion-pkg
sudo install -Dm755 "$(find zylaxion-pkg -type f -name zylaxion | head -1)" /usr/bin/zylaxion
```

Releases are arch-baseline builds: every v80-line package is
x86-64-v3 (AVX2/FMA/BMI) or x86-64-v4 (AVX-512), both dynamically linked
against glibc — two tarballs per tag. Pick v3 when in doubt.

The musl lane is documented dormant. zylaxion's audio path runs
through cpal, whose Linux backend is ALSA (`libasound.so`). No
musl-compatible static `libasound.a` ships with any mainstream
distribution, and building one is a maintenance burden disproportionate
to the value. The lane wakes when cpal gains a pure-Rust backend,
PipeWire gains a pure-Rust native binding, or ALSA ships a musl static
archive. Until then: dormant, documented, honest.

### Install from crates.io

`cargo install zylaxion --locked` builds from the published source
tarball and lands the **full-featured binary** — the eBPF tap embedded
— on a plain **stable toolchain: no nightly, no bpf-linker, no
bootstrap** ever touches the installing machine. The crate ships
**`ebpf-prebuilt/`** instead: the maintainer-built kernel object with
its provenance manifest, which `build.rs` stages. The lane enforces
itself twice: build-time auto-regeneration and a commit-time parity
gate.

### Install from source

Building needs the pinned Rust toolchain, `pkg-config`, `libasound2-dev`,
and — because the eBPF tap is unconditional (the unkillable-ebpf law:
no feature gate exists) — the dated nightly pin + bpf-linker on the
BUILD host. One command readies the whole host:

```bash
git clone https://github.com/oxyzenQ/zylaxion.git
cd zylaxion
./scripts/dev/bootstrap-ebpf.sh
cargo pro-linux-amd64-gnu --locked
```

The alias is the build-from-source law in action — a named,
parity-checked shape, never a bare `cargo build --release`. On an
x86-64-v3/v4 CPU the tuned tiers are `cargo pro-linux-amd64-v3-gnu`
and `cargo pro-linux-amd64-v4-gnu`, or let the installer's CPU-level
detection pick for you.

The one-command installer detects the CPU's x86-64 level, builds the
matching parity-checked shape through the cargo aliases, and lands the
system destinations — binary to `/usr/bin`, the boot oneshot and the
login hand-over to `/etc/systemd/system`:

```bash
./scripts/package/install-on-my-machine.sh
```

### Uninstall

```bash
./scripts/package/uninstall.sh
```

`sudo zylaxion detach` unpins the kernel side, and there is nothing
else to clean.

### Usage

Every verb, once — flags live in `zylaxion --help`, the deep truth in
`zylaxion status`:

```bash
sudo zylaxion attach             # Plant the kernel-side tap (once per boot)
zylaxion on                      # Sound on (spawns the stateless player)
zylaxion on --preset cherryMX    # Sound on with a compiled-registry preset
zylaxion preset --switch topre   # Switch the preset live while playing
zylaxion volume --percent 60     # Retune the master volume live
zylaxion off                     # Sound off (player self-exits, zero processes)
zylaxion status                  # Kernel verdict, tap residency, player state
zylaxion status --verbose        # + kernel-side typing telemetry
sudo zylaxion detach             # Unpin everything
zylaxion doctor                  # System health diagnostic
zylaxion list-presets            # List the 24 compiled presets + active one
zylaxion list-backends           # Show available audio backends
zylaxion --check-update          # Check for latest GitHub release
zylaxion -V                      # Show version + build info
```

A verb that cannot be honored is a hard error — red `error:` block,
exit 1, with a did-you-mean tip for a typo'd preset name. There is no
silent fallback.

The hidden and advanced vocabulary — the short aliases (`-s`, `-p`,
`-v`), the environment surface (`ZYLAXION_PRESET`, `ZYLAXION_VOLUME`,
`NO_COLOR`, `RUST_LOG`), and the internal `_play` verb — is
documented completely in the Pro mode section of `zylaxion --help`.

### Configuration

There is no configuration file. The Unix configuration surface itself:

1. `--preset <name>` on the CLI (highest priority)
2. `ZYLAXION_PRESET=<name>` in the environment (persistent selection)
3. the compiled default preset (`technical`) if neither names one

The master output volume follows the same law:
`ZYLAXION_VOLUME=1.5` (headphones) overrides the compiled
laptop-speaker default of 5.5. An invalid value is a hard error.

A preset is chosen when the sound starts; while playing, switch it
live with `zylaxion preset --switch <name>` (one map write — the
player's watcher thread swaps the DSP model in place).

### Built-in sound presets

zylaxion ships **24 presets** — 6 "character" presets, 3 switch-type
presets, 10 high-fidelity legendary keyboard profiles, and 5 sci-fi /
futuristic presets. Every preset rides the same 3-layer DSP engine and
the same micro-randomization law.

#### Character presets (general use)

| Preset    | Description                                      |
|-----------|--------------------------------------------------|
| technical | Crisp, loud, punchy. Cherry MX Blue click style. **Compiled default.** |
| cherryMX  | Balanced reference. General MX Blue/Brown.       |
| classic   | Deeper, resonant. Warm bucklespring tone.        |
| studio    | Softer attack, longer decay. Office-friendly.     |
| elegant   | Very soft, muffled. Low-profile keyboards.       |
| whisper   | Extremely quiet, short decay. Libraries/meetings. |

Plus three switch-type presets: `linear`, `tactile`, `clicky`.

#### Legendary keyboard profiles

| Preset             | Switch Type         | Character                                              |
|--------------------|---------------------|--------------------------------------------------------|
| `ibm_model_m`      | Buckling spring     | Deep, very resonant, long ring. The showcase legend. |
| `topre`            | Electrocapacitive   | Deep "thock", smooth, low resonance. Realforce/HHKB.  |
| `cherry_mx_black`  | Linear (heavy)      | Smooth thud, faint spring ping. ~80g actuation.       |
| `cherry_mx_clear`  | Tactile (heavy)     | Pronounced bump, louder than Brown. ~65g actuation.   |
| `cherry_mx_silver` | Linear (low-pro)    | Fast, snappy bottom-out. Designed for gaming.         |
| `buckling_spring`  | Buckling spring     | Sharper & brighter than Model M. Think Model F.       |
| `alps_skcm`        | Tactile (vintage)   | Complicated ALPS — loud, hollow housing ring.         |
| `gateron_yellow`   | Linear (thocky)     | Lubed linear with PBT caps. Deep, rounded thock.      |
| `zealios_v2`       | Tactile (sharp)     | Almost clicky-feeling bump. Enthusiast tactile.       |
| `rotary_encoder`   | Encoder detent      | Not a keyboard — a smooth metallic knob click.        |

#### Sci-Fi / Futuristic presets

| Preset             | Character                                                  |
|--------------------|------------------------------------------------------------|
| `cyber_deck`       | High-pitched metallic pings. Hollywood "future terminal".  |
| `oceanic`          | Deep underwater rumble, long bass sustain, muffled thud.   |
| `glass_tactile`    | Tapping a crystal wine glass — pure ringing tone.          |
| `retro_typewriter` | Sharp typebar strike + cast-iron frame ring (no bell).     |
| `neon_pulse`       | Cyberpunk UI button — heavy granular texture on a click.   |

The Zactrix Engine that voices all 24 presets is LTS-frozen (the
certification and its six proofs live in
[docs/ZACTRIX_LTS.md](docs/ZACTRIX_LTS.md), the full security audit
in [docs/ZACTRIX_SECURITY.md](docs/ZACTRIX_SECURITY.md)): the engine
changes only for a security fix, a compatibility fix, or a
measurable improvement proven by the audit harness.

## Limitations (honest)

- **Root once per boot** — `sudo zylaxion attach` plants the tap;
  everything after that is unprivileged.
- **Kernel 5.15+ with BPF available** — lockdown / integrity modes
  that block BPF are refused with the exact reason.
- **The kernel's own unprivileged-bpf gate** — distros that build
  `CONFIG_BPF_UNPRIV_DEFAULT_OFF=y` in boot with
  `kernel.unprivileged_bpf_disabled=2`: the control verbs refuse
  classified and `zylaxion doctor` carries the posture line.
- **Audio rides the desktop's bridge** — cpal → PipeWire / PulseAudio
  / ALSA. There is no direct JACK backend and no network audio.
- **The musl lane is dormant** — a fully static build waits on a
  static answer for the alsa C library.
- **A keystroke is a scancode, never a letter** — sound class and
  stereo pan come from key position; layout awareness is out of scope
  by design — it is the zero-leakage posture's first wall.
- **One engine for every keyboard** — the kernel tap sees
  `input_event()`, not devices, so two plugged-in keyboards sound the
  same.
- **CI has no ears** — runners carry no audio and no input hardware.
  A human's ears on real hardware stay the owner's verification.

### The nightly eBPF toolchain (honest)

The eBPF build dependencies (`aya-ebpf` on a pinned nightly rustc,
plus bpf-linker) are experimental — but they are quarantined at
BUILD time. The BPF object is cross-compiled, validated, and embedded
inside the single release binary, so a deployment needs only a kernel
and the one file: no rustup, no nightly, no LLVM.

## Security & Privacy

zylaxion is designed with a **zero-leakage** posture. The short
version: **your keystrokes never leave the process.**

### What zylaxion reads

- **Hardware scancodes** — the physical key location. Not an ASCII
  character. zylaxion does not translate scancodes into letters. It
  feeds them straight into the DSP engine to choose a stereo pan
  position and trigger a click sound.
- The kernel's input core via a passive eBPF probe on `input_event()`.

### What zylaxion does NOT do

- **Does not log scancodes** — not at `info`, not at `debug`, not at
  `trace`. Audit the codebase: `grep -r 'scancode' src/ | grep -E
  'log::|println!|eprintln!'` returns zero hits.
- **Does not store keystrokes** — nothing is written to any
  filesystem: the state store is pinned BPF maps, and the per-key
  press counters are aggregate counts, never sequences.
- **Does not transmit keystrokes** — no telemetry, no analytics, no
  crash reports. The only outbound network call is `--check-update`.
- **Does not accept inbound network connections** — no TCP listener,
  no HTTP server, no Unix socket, no IPC protocol.

### Verifying the policy yourself

```bash
# Should print zero matches in production paths:
grep -rn 'scancode' src/ examples/ --include='*.rs' | \
    grep -E 'log::|println!|eprintln!' | \
    grep -v 'examples/'

# Introspect the entire control plane with stock kernel tooling:
bpftool map dump pinned /sys/fs/bpf/zylaxion/control
# The kernel IS the config, the config IS introspectable.
```

## Architecture

The daemonless control plane — the kernel owns the state, the tap is
passive, the player is disposable:

```
                 KERNEL (resident, ~1-2us per keystroke)
 ┌────────────────────────────────────────────────────────────┐
 │  eBPF tap: fentry/kprobe input_event() (the ears)          │
 │    │  filter: EV_KEY, press/release, mapped keys only,     │
 │    │  enabled gate, per-key stats (BPF_ATOMIC fetch-add)   │
 │    ▼                                                       │
 │  pinned maps on /sys/fs/bpf/zylaxion/ (the brain):         │
 │    control (enabled, preset, volume, player pid)           │
 │    keymap  (keycode -> sound class)                        │
 │    events  (ring buffer)      stats (per-key counters)     │
 └────────────────────────────┬───────────────────────────────┘
                              │ epoll (zero idle CPU)
                              ▼
                 PLAYER (alive only while sound plays)
 ┌────────────────────────────────────────────────────────────┐
 │  zylaxion _play — stateless, owns nothing (the mouth):     │
 │  drain ring buffer -> core Orchestrator -> VoicePool ->    │
 │  ringbuf -> cpal callback -> PipeWire/ALSA -> speakers     │
 └────────────────────────────────────────────────────────────┘

 CLI (lives < 5 ms, then exits):
   zylaxion on | off | preset | volume | status  -> map writes/reads
   sudo zylaxion attach | detach                 -> plant / remove
```

## Philosophy

**The boring and stealth.**

zylaxion stays out of the user's way while doing serious work
underneath: it plants a passive kernel probe, it synthesizes sound
from mathematics, and it owns nothing. The interface rarely changes.
Features don't explode. Every release makes it slightly more stable,
slightly faster, slightly easier to maintain.

### What zylaxion IS

- **Single CLI binary** — no daemon, no resident service, no config file
- **Pure math synthesis** — no audio samples, no wavetables, no decoders
- **Pure eBPF input** — no `/dev/input`, no evdev grab, no udev rules
- **Predictable behavior** — same input → same sound, every time
- **Linux-only at runtime** — BSD/macOS and Windows are out of scope, ever

### What zylaxion will NEVER be

- No sample packs, no downloadable sound fonts, no assets to load
- No daemon mode, no background supervision
- No `config.toml`, no dotfile, no registry of any kind
- No keystroke logging, telemetry, or analytics
- No REST API, no network control plane

### Stable API (from v80.0.0)

The CLI surface is frozen. No breaking changes to commands, flags, or
output format. Future releases focus on bug fixes, kernel compatibility,
and internal performance.

### Release channels

Stable releases are tagged `vX.Y.Z`. Pre-release builds are restricted
to five channels — `dev`, `nightly`, `alpha`, `beta`, `rc`. CI rejects
any other suffix and never marks a pre-release as "latest".

### Branches

| Branch | Purpose | Status |
|--------|---------|--------|
| `main` | Pure math synthesis + the kernel-side eBPF tap | active |

## Contributing

PRs and issues are welcome. The bar is the gate suite — build, test,
conventions, and the script inventory are documented once in
[CONTRIBUTING.md](CONTRIBUTING.md). Contributions land under the
[Contributor License Agreement](CLA.md); the DCO-style
`Signed-off-by:` line is the acceptance.

## Security Policy

zylaxion runs a kernel probe and voices every keystroke on the machine,
so security is part of the product. Found something? **Report it
privately** — the policy and supported versions live in
[SECURITY.md](SECURITY.md).

## Documentation

- [Documentation Map](docs/README.md) — every document in the tree
- [Philosophy](docs/PHILOSOPHY.md) — "the boring and stealth"
- [The eBPF-only bet](docs/EBPF_ONLY_VS_DAEMON.md) — the honest
  pros-and-cons ledger of the daemon deletion
- [Kernel Compatibility](docs/KERNEL_COMPATIBILITY.md) — the 5.15
  floor, BTF/lockdown behavior
- [Release Verification](docs/VERIFY_RELEASE.md) — checksum + GPG
  walkthrough
- [Rules](docs/RULES.md) — the enforced conventions
- [Contributing Guide](CONTRIBUTING.md) — gates, conventions
- [Contributor License Agreement](CLA.md) — CLA terms
- [Security Policy](SECURITY.md) — private reporting
- [Commercial License](COMMERCIAL_LICENSE.md) — tiers, payment
- [Trademark](TRADEMARK.md) — the Marks and forking policy
- [Changelog](CHANGELOG.md) — the notable-changes record

## Test Results

The suite is the contract. The headless probes run in the claims
battery, the micro-VM legs prove the rest inside a real kernel:

| Harness | What it proves | One command |
|---------|----------------|-------------|
| [supermassive rootless self-test](scripts/supermassive/supermassive-test.sh) | The env probes, the CLI/UX battery, and the static claims | `./scripts/supermassive/supermassive-test.sh --self-test` |
| [supermassive full battery](scripts/supermassive/supermassive-test.sh) | The tap lifecycle, the night-dinner-3 repro, a real user typing through uinput, telemetry, PCM witness | `sudo ./scripts/supermassive/supermassive-test.sh --full` |
| KVM micro-VM E2E (CI) | The zero-to-hero chain inside a real kernel (5.15 floor & latest) | `.github/workflows/supermassive.yml` |
| [the workspace test tree](scripts/build.sh) | Unit + doc tests, plus fmt and clippy | `./scripts/build.sh check-all -q` |
| [the gate suite](scripts/gate-keepers.sh) | Permissions, SPDX headers, LOC caps, language, release parity | `./scripts/gate-keepers.sh` |

### The claims battery

The extraordinary claims get their own independent verification lane
(`scripts/supermassive/sm_static.py`):

- **Pure math** — a hand-rolled ELF64 `DT_NEEDED` parser proves zero
  audio decoders are linked.
- **eBPF** — the binary is scanned for embedded ELF-BPF objects
  carrying the `input_event` attach target.
- **No daemon, no bloat** — the binary is swept for daemon-era strings
  and the transitive lockfile is swept for daemon-era crates.

A green Actions run is the same verdict these commands produce locally.

## Release Verification

Each release ships **three** checksums: classical SHA-512 +
quantum-resistant BLAKE3-256 + SHAKE256, and — when the owner's
signing key is configured — a GPG detached signature (`.asc`):

```bash
# Classical
sha512sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.sha512sum

# BLAKE3
b3sum -c zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.b3sum

# SHAKE256 (NIST PQ standard, via Python)
COMPUTED=$(python3 -c "import hashlib; print(hashlib.shake_256(open('zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz','rb').read()).hexdigest(64))")
EXPECTED=$(awk '{print $1}' zylaxion-vX.Y.Z-linux-amd64-v3-gnu.tar.gz.shake256)
[ "$COMPUTED" = "$EXPECTED" ] && echo "OK" || echo "FAILED"
```

The full walkthrough lives in [docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md).

## Support

Built and maintained independently by **rezky_nightky (oxyzenQ)**.
If this project made your keyboard sound the way it always should have,
you can support future maintenance here:

[![Support me on Ko-fi](https://ko-fi.com/img/githubbutton_sm.svg)](https://ko-fi.com/rezky)

### Crypto donations

- **Solana** — `SOL` / `USDT` (SPL): `88umzS7abaToaGQVgTVXt5SnuvcjTw2jPSM6Ha2JYmXM`
- **Ethereum** — `ETH` / `USDT` / `USDC` (ERC-20): `0x1bCbA21c07B5636a942De27AA7Ee8283cEDb4C3D`
- **Bitcoin** — `BTC` (Taproot, P2TR, `bc1p`-prefixed): `bc1p88nqysn4p8u9zxwz2pyxs5pl77wllcrk6ca2r2l3ryr3863hxkys5vdkze`

Support is optional. The project remains open-source.

## Intellectual Property & Trademark

**zylaxion** is the exclusive intellectual property of
**rezky_nightky (oxyzenQ)**.

- Source code: licensed under **GPL-3.0-only** (see [LICENSE](LICENSE)).
- Name, logo, and branding ("the Marks"): governed by
  [TRADEMARK.md](TRADEMARK.md). The Marks are NOT covered by the GPL and
  are reserved by the owner.
- This project is **NOT for sale**. Unauthorized rebranding, relicensing,
  or source-code theft is strictly prohibited.

**Forking policy** (full text in [TRADEMARK.md §4](TRADEMARK.md)):
forks of zylaxion MUST use a different project name, MUST use
different branding, MUST preserve the GPL-3.0-only license, and MUST
clearly state the fork is derived from zylaxion but is NOT endorsed by
rezky_nightky (oxyzenQ).

## Commercial Licensing

Companies using zylaxion in production need a commercial license.
zylaxion is dual-licensed: **GPL-3.0-only** for open-source use, and a
**Commercial License** for proprietary use.

| Tier       | Price          | Target                                              |
| ---------- | -------------- | --------------------------------------------------- |
| Personal   | Free (GPL-3.0) | Hobby, personal, non-commercial, contributions      |
| Individual | $99/year       | Solo devs, freelancers, revenue < $100K/year        |
| Business   | $2,199/year    | Individual businesses and small teams (SMB)         |
| Company    | $9,199/year    | Companies and enterprise, OR redistribution rights  |

**Unauthorized use is theft.** The honest paths are exactly two:
comply with GPL-3.0-only (free, first-class), or buy the tier your
revenue honestly matches.

Payment is USD-pegged crypto (Solana / Ethereum / Bitcoin). Licensing
contact: [with.rezky@gmail.com](mailto:with.rezky@gmail.com).

## License

Dual-licensed: **GPL-3.0-only** for open-source use (see
[LICENSE](LICENSE)), or the **Commercial License** for proprietary and
commercial use (see [COMMERCIAL_LICENSE.md](COMMERCIAL_LICENSE.md)).

---

© 2026 rezky_nightky (oxyzenQ). All rights reserved. zylaxion and the
zylaxion logo are trademarks of rezky_nightky (oxyzenQ).

<p align="center">
  <em>No samples. No daemon. No config file. Just math.</em>
</p>

<p align="center">
  <em>built with curiosity</em>
</p>

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
