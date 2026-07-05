<p align="center">
  <img src="assets/zylaxion-logo-master.png" alt="zylaxion logo" width="260">
</p>

<h1 align="center">zylaxion</h1>

<p align="center">
  <strong>Linux-first real-time mechanical keyboard acoustic engine.</strong>
</p>

<p align="center">
  Pure procedural synthesis. Zero audio samples. Low-latency.
</p>

<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support on Ko-fi">
  </a>
</p>

---

Zylaxion transforms every keystroke into a spatially accurate mechanical keyboard sound using Linux's `evdev` interface and real-time audio output through `cpal` and PipeWire.

Instead of replaying recorded samples, every sound is **synthesized mathematically** using noise excitation, TPT State Variable Filters, resonance modeling, and exponential decay envelopes. This pure procedural approach means:

- **Zero audio samples** — no wavetables, no recorded clips, no sample libraries. The entire sound engine is mathematical computation.
- **Ultra-lightweight** — the binary is self-contained; no audio assets to load, store, or manage.
- **Infinitely tunable** — every parameter (filter frequencies, resonance Q, decay coefficients, spring mix) is a number in `config.toml`, adjustable in real time.
- **Deterministic** — the same key + same config always produces the same waveform. No random variation from sample playback.

No audio files. No sample libraries. No wavetable playback. Just math.

**Works on Wayland, X11, and Linux TTY.**

## Features

- **Pure procedural synthesis** — all sounds generated from math, not samples.
  Dual TPT SVF filters model the click transient and spring resonance of
  real mechanical key switches with zero audio assets.
- **Real-time audio** — cpal + PipeWire with interrupt-driven ring buffer
  rendering. Typical latency under 3 ms.
- **Polyphonic voice pool** — 16 simultaneous voices with oldest-first voice
  stealing, stereo panning based on key position, and exponential decay.
- **Central `config.toml`** — single source of truth for all acoustic
  DSP parameters (click frequency, resonance, spring mix, decay
  coefficient, per-key overrides). Edit, save, and the running daemon
  auto-reloads within 1 second — no restart needed. Presets for 5 sound
  characters (`technical`, `classic`, `studio`, `elegant`, `whisper`)
  are documented as copy-paste blocks at the top of the file.
- **Daemon mode** — POSIX-compliant daemonization with signal handling,
  PID file with recycling protection, and Unix Domain Socket IPC for
  remote stop/status commands.
- **Zero system dependencies** — only requires a C compiler for linking.
  Audio via PipeWire/PulseAudio (system), input via libinput (kernel evdev).
- **Linux-first** — built for the Linux kernel's evdev subsystem.
  No X11 or Wayland dependency; input capture works everywhere.

## Installation

### From source

Requires: Rust toolchain ([rustup.rs](https://rustup.rs/)),
`pkg-config`, `libasound2-dev`, `libinput-dev`, `libudev-dev`.

> Zylaxion plays audio through `cpal`, which talks to PipeWire /
> PulseAudio / ALSA via the OS's default audio bridge. No PipeWire
> native development headers are required — only `libasound2-dev`
> (ALSA backend that cpal links against).

```bash
git clone https://github.com/oxyzenQ/zylaxion.git
cd zylaxion

# Build the release binary
cargo build --release --locked

# Install (binary + config.toml go to ~/.local by default)
./scripts/install.sh
```

To install to a custom prefix:

```bash
PREFIX="$HOME/.local" ./scripts/install.sh
```

The installer copies the binary to `${PREFIX}/bin/zylaxion` and the
central `config.toml` to `${PREFIX}/share/zylaxion/config.toml`. It
does **not** run `cargo build` — build first, then install.

Since v3.0.0 the installer also deploys a **systemd user service** to
`~/.config/systemd/user/zylaxion.service`. To enable auto-start on login:

```bash
systemctl --user enable --now zylaxion
systemctl --user status  zylaxion
journalctl --user -u zylaxion -f   # live logs
```

> **Note:** your user must be in the `input` group for keyboard access:
> `sudo usermod -aG input $USER && log out/in`

### Download a release binary

Pre-built binaries are attached to each [GitHub release](https://github.com/oxyzenQ/zylaxion/releases).
Download, make executable, and copy to a directory on your PATH:

```bash
wget https://github.com/oxyzenQ/zylaxion/releases/latest/download/zylaxion
chmod +x zylaxion
install -Dm755 zylaxion "$HOME/.local/bin/zylaxion"
```

### Release Verification

Each release ships **three** checksums: classical SHA-512 + quantum-resistant
BLAKE2b-512 + SHAKE256. Full instructions in
[docs/VERIFY_RELEASE.md](docs/VERIFY_RELEASE.md).

```bash
# Classical (universal)
sha512sum -c zylaxion-vX.Y.Z-linux-amd64-gnu.tar.gz.sha512sum

# Quantum-resistant — BLAKE2b (fastest, in coreutils)
b2sum -c zylaxion-vX.Y.Z-linux-amd64-gnu.tar.gz.b2sum

# Quantum-resistant — SHAKE256 (NIST PQ standard, via openssl)
openssl dgst -shake256 zylaxion-vX.Y.Z-linux-amd64-gnu.tar.gz
# Compare hash with: cat zylaxion-vX.Y.Z-linux-amd64-gnu.tar.gz.shake256
```

## Usage

```
zylaxion start                    # Foreground mode (Ctrl+C to quit)
zylaxion start --preset cherryMX  # Override active preset from CLI
zylaxion daemon                   # Background daemon mode
zylaxion stop                     # Stop a running daemon
zylaxion status                   # Check if daemon is running
zylaxion doctor                   # System health diagnostic
zylaxion testconf                 # Validate config.toml syntax + ranges
zylaxion list-presets             # List available presets + active one
zylaxion list-backends            # Show available audio backends
```

### Configuration

The central `config.toml` controls every aspect of the click sound:
filter frequencies, resonance (Q), spring mix level, decay rate, and
amplitude, plus optional `[[keys]]` per-scancode overrides. The file
is loaded from (first found wins):

1. `~/.config/zylaxion/config.toml` — user-local override
2. `/etc/zylaxion/config.toml` — system config
3. `/usr/local/share/zylaxion/config.toml` — installed default
4. `./config.toml` — relative to CWD (development)
5. Hardcoded default — always available

#### Active preset selection

The file defines multiple named `[preset.NAME]` tables. The active
preset is determined by:

1. `--preset <name>` on the CLI (highest priority — overrides everything)
2. `tuning = "<name>"` in the `[preset]` table of `config.toml`
3. `"technical"` (hardcoded default) if neither is set

If the resolved preset does NOT exist in `config.toml`, the program
prints a clear error listing the available presets and exits — there
is **no silent fallback**. This prevents accidental misconfiguration.

#### Auto-reload

The running daemon polls `config.toml`'s mtime every 1 second and
auto-reloads on change. If you started without `--preset`, changing
`tuning = "cherryMX"` and saving causes an immediate swap to the
cherryMX preset — no restart needed. If you started with `--preset`,
the CLI value wins and `tuning` changes are ignored (you'd need to
restart with a different `--preset`).

Run `zylaxion testconf` after editing to catch TOML typos and
out-of-bounds DSP values before they affect the running daemon.

### Built-in sound presets

Zylaxion ships with **24 presets** — 6 "character" presets, 3
switch-type presets, **10 high-fidelity legendary keyboard profiles**
(v4.0.0+) tuned to mimic real switches, and **5 sci-fi / futuristic
presets** (v4.1.0+) that don't mimic anything real. Since v4.2.0,
**every preset** uses a 3-layer DSP engine (click + spring + housing
"thock") for realistic acoustic body. Since v5.0.0, every keypress
applies **micro-randomization** (±1.5% pitch drift, ±5% amplitude
drift, unique noise seed) so the same key never sounds identical twice
— breaking the "uncanny valley" of deterministic synthesis.

#### Character presets (general use)

| Preset    | Description                                      |
|-----------|--------------------------------------------------|
| technical | Crisp, loud, punchy. Cherry MX Blue click style. |
| cherryMX  | Balanced reference. General MX Blue/Brown.       |
| classic   | Deeper, resonant. Warm bucklespring tone.        |
| studio    | Softer attack, longer decay. Office-friendly.     |
| elegant   | Very soft, muffled. Low-profile keyboards.       |
| whisper   | Extremely quiet, short decay. Libraries/meetings. |

Plus three switch-type presets: `linear`, `tactile`, `clicky`.

#### Legendary keyboard profiles (v4.0.0+)

Each of the following presets was hand-tuned against reference
recordings of the actual hardware. The DSP parameters (click
frequency, resonance Q, decay coefficient, ambient rattle) are
adjusted to capture the signature character of each switch. Three
presets (`ibm_model_m`, `topre`, `alps_skcm`) received a
realism tuning pass in v4.1.0 — softer Topre attack, longer IBM
Model M ring, deeper ALPS housing.

| Preset             | Switch Type         | Character                                              |
|--------------------|---------------------|--------------------------------------------------------|
| `ibm_model_m`      | Buckling spring     | Deep, very resonant, long ring. **Default since v4.0.0.** |
| `topre`            | Electrocapacitive   | Deep "thock", smooth, low resonance. Realforce/HHKB.  |
| `cherry_mx_black`  | Linear (heavy)      | Smooth thud, faint spring ping. ~80g actuation.       |
| `cherry_mx_clear`  | Tactile (heavy)     | Pronounced bump, louder than Brown. ~65g actuation.   |
| `cherry_mx_silver` | Linear (low-pro)    | Fast, snappy bottom-out. Designed for gaming.         |
| `buckling_spring`  | Buckling spring     | Sharper & brighter than Model M. Think Model F.       |
| `alps_skcm`        | Tactile (vintage)   | Complicated ALPS — loud, hollow housing ring.         |
| `gateron_yellow`   | Linear (thocky)     | Lubed linear with PBT caps. Deep, rounded thock.      |
| `zealios_v2`       | Tactile (sharp)     | Almost clicky-feeling bump. Enthusiast tactile.       |
| `rotary_encoder`   | Encoder detent      | Not a keyboard — a smooth metallic knob click.        |

#### Sci-Fi / Futuristic presets (v4.1.0+)

These five presets are completely original acoustic signatures —
they don't mimic real keyboards. Each pushes the DSP parameter
ranges to extremes for cyberpunk aesthetic, gaming immersion, or
just fun. Perfect for streaming setups with a sci-fi theme, game
developers wanting UI key sounds, or anyone who wants a unique
typing experience no human has heard before.

| Preset             | Character                                                  |
|--------------------|------------------------------------------------------------|
| `cyber_deck`       | High-pitched metallic pings. Hollywood "future terminal".  |
| `oceanic`          | Deep underwater rumble, long bass sustain, muffled thud.   |
| `glass_tactile`    | Tapping a crystal wine glass — pure ringing tone.          |
| `retro_typewriter` | Sharp typebar strike + cast-iron frame ring (no bell).     |
| `neon_pulse`       | Cyberpunk UI button — heavy granular texture on a click.   |

Try them all:

```bash
zylaxion list-presets
zylaxion start --preset ibm_model_m
zylaxion start --preset topre
zylaxion start --preset alps_skcm
zylaxion start --preset glass_tactile
zylaxion start --preset neon_pulse
```

## Security & Privacy

Zylaxion is designed with a **zero-leakage** posture. The short
version: **your keystrokes never leave the process.**

### What Zylaxion reads

- **Hardware scancodes** — the physical key location on the keyboard
  (e.g. scancode `30` is the home row key under QWERTY's left hand).
  This is *not* an ASCII character. Zylaxion does not translate
  scancodes into letters, symbols, or text. It feeds them straight
  into the DSP engine to choose a stereo pan position and trigger a
  click sound.
- The kernel's `evdev` interface via `libinput` — same data any
  keyboard-aware Linux program can read.

### What Zylaxion does NOT do

- **Does not log scancodes** — not at `info`, not at `debug`, not at
  `trace`. The `KeyEvent` struct exists to drive the DSP engine, not
  for human inspection. Audit the codebase: `grep -r 'scancode'
  crates/ | grep -E 'log::|println!|eprintln!'` returns zero hits
  in production paths.
- **Does not store keystrokes** — there is no on-disk log, no
  history file, no analytics. The only thing Zylaxion writes to disk
  is its PID file (`$XDG_RUNTIME_DIR/zylaxion.pid`) and the daemon's
  Unix socket (`$XDG_RUNTIME_DIR/zylaxion.sock`).
- **Does not transmit keystrokes** — no telemetry, no analytics, no
  crash reports. The only outbound network call is the explicit
  `zylaxion --check-update` subcommand, which fetches the latest
  GitHub release tag and exits. No background phone-home.
- **Does not accept inbound network connections** — there is no TCP
  listener, no HTTP server. The only IPC channel is a Unix domain
  socket bound to `$XDG_RUNTIME_DIR/zylaxion.sock` with file mode
  `0o600` (owner-only read/write). Other users on the system cannot
  send `stop` / `status` commands to your daemon.

### Why this matters

A naive "acoustic keyboard" implementation that logged scancodes for
debugging would be a side-channel leak: malware with read access to
`journalctl --user -u zylaxion` could reconstruct typed passwords by
mapping scancodes back through the user's keyboard layout (QWERTY,
Dvorak, etc.). Zylaxion's zero-trust logging policy eliminates this
attack surface — there is nothing in the logs to reconstruct.

### Verifying the policy yourself

```bash
# Should print zero matches in production paths (excluding examples/
# which carry an explicit --dump-scancodes opt-in for hardware
# debugging):
grep -rn 'scancode' crates/ --include='*.rs' | \
    grep -E 'log::|println!|eprintln!' | \
    grep -v 'examples/'

# Verify the IPC socket has mode 0o600 after starting the daemon:
ls -l "$XDG_RUNTIME_DIR/zylaxion.sock"
# Expected: srw------- 1 user user 0 ... zylaxion.sock
```

## Building

```bash
# Run all checks (fmt + clippy + test)
./scripts/build.sh --check-all

# Build release binary
cargo build --release --locked

# Bump workspace version
./scripts/version-to.sh v5.0.1
```

## Uninstall

```bash
./scripts/uninstall.sh
```

## Architecture

```
zylaxion-input          zylaxion-core           zactrix-engine          zylaxion-output
(The Ears)              (The Brain)            (Zactrix Engine)        (The Mouth)
───────────             ──────────             ───────────────         ─────────────
LibinputSource ──►      recv_timeout()
   KeyEvent               │
                     trigger / release
                           │
                           ▼
                     VoicePool::process()
                           │
                     [[f32; 2]] batch
                           │
                           ▼
                     AudioSink::write_batch()
                                          ringbuf ──►  cpal callback
```

## Intellectual Property & Trademark

**zylaxion** is the exclusive intellectual property of
**rezky_nightky (oxyzenQ)**.

- Source code: licensed under **GPL-3.0-only** (see [LICENSE](LICENSE)).
- Name, logo, and branding ("the Marks"): governed by
  [TRADEMARK.md](TRADEMARK.md). The Marks are NOT covered by the GPL and
  are reserved by the owner.
- This project is **NOT for sale**. Unauthorized rebranding, relicensing,
  or source-code theft is strictly prohibited.

For trademark licensing or written permission, contact
**rezky_nightky (oxyzenQ)** — https://github.com/oxyzenQ.

---

© 2026 rezky_nightky (oxyzenQ). All rights reserved.
