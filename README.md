<p align="center">
  <img src="assets/zylaxion-logo-master.png" alt="Zylaxion logo" width="260">
</p>

<h1 align="center">zylaxion</h1>

<p align="center">
  <strong>Linux-first real-time mechanical keyboard acoustic engine.</strong>
</p>

<p align="center">
  Pure procedural synthesis. Zero audio samples. Ultra-low latency.
</p>

<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://img.shields.io/badge/Ko--fi-support-7C3AED?style=flat-square&logo=kofi&logoColor=white&labelColor=111827" alt="Support on Ko-fi">
  </a>
</p>

---

Zylaxion transforms every keystroke into a spatially accurate mechanical keyboard sound using Linux's `evdev` interface and real-time audio output through `cpal` and PipeWire.

Instead of replaying recorded samples, every sound is synthesized mathematically using noise excitation, TPT State Variable Filters, resonance modeling, and exponential decay envelopes.

No audio files. No sample libraries. No wavetable playback.

**Works on Wayland, X11, and Linux TTY.**

## Features

- **Pure procedural synthesis** — all sounds generated from math, not samples.
  Dual TPT SVF filters model the click transient and spring resonance of
  real mechanical key switches with zero audio assets.
- **Real-time audio** — cpal + PipeWire with interrupt-driven ring buffer
  rendering. Typical latency under 3 ms.
- **Polyphonic voice pool** — 16 simultaneous voices with oldest-first voice
  stealing, stereo panning based on key position, and exponential decay.
- **5 acoustic profiles** — `technical`, `classic`, `studio`, `elegant`,
  `whisper`. TOML-configurable DSP parameters (click frequency, resonance,
  spring mix, decay coefficient). Custom profiles supported via
  `~/.config/zylaxion/profiles/`.
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

```bash
git clone https://github.com/oxyzenQ/zylaxion.git
cd zylaxion

# Build the release binary
cargo build --release --locked

# Install (binary + profiles go to /usr/local by default)
sudo ./scripts/install.sh
```

To install to a custom prefix:

```bash
PREFIX=/usr sudo ./scripts/install.sh
```

The installer copies the binary to `${PREFIX}/bin/zylaxion` and the
acoustic profile TOMLs to `${PREFIX}/share/zylaxion/profiles/`. It
does **not** run `cargo build` — build first, then install.

> **Note:** your user must be in the `input` group for keyboard access:
> `sudo usermod -aG input $USER && log out/in`

### Download a release binary

Pre-built binaries are attached to each [GitHub release](https://github.com/oxyzenQ/zylaxion/releases).
Download, make executable, and copy to a directory on your PATH:

```bash
wget https://github.com/oxyzenQ/zylaxion/releases/latest/download/zylaxion
chmod +x zylaxion
sudo install -Dm755 zylaxion /usr/local/bin/zylaxion
```

## Usage

```
zylaxion start                    # Foreground mode (Ctrl+C to quit)
zylaxion start --profile whisper  # With acoustic profile
zylaxion daemon                   # Background daemon mode
zylaxion daemon --profile classic
zylaxion stop                     # Stop a running daemon
zylaxion status                   # Check if daemon is running
zylaxion doctor                   # System health diagnostic
zylaxion list-profiles            # Show available acoustic profiles
zylaxion list-backends            # Show available audio backends
```

### Acoustic profiles

Profiles are TOML files that control every aspect of the click sound:
filter frequencies, resonance (Q), spring mix level, decay rate, and
amplitude. They are loaded from (first found wins):

1. `~/.config/zylaxion/profiles/<name>.toml` — user-local overrides
2. `/usr/local/share/zylaxion/profiles/<name>.toml` — installed data
3. `/usr/share/zylaxion/profiles/<name>.toml` — system data
4. `./profiles/<name>.toml` — relative to CWD (development)
5. Hardcoded default — always available

### Built-in profiles

| Profile    | Description                                      |
|------------|--------------------------------------------------|
| technical  | Crisp, loud, punchy. Cherry MX Blue click style. |
| classic    | Deeper, resonant. Warm bucklespring tone.        |
| studio     | Softer attack, longer decay. Office-friendly.     |
| elegant    | Very soft, muffled. Low-profile keyboards.       |
| whisper    | Extremely quiet, short decay. Libraries/meetings. |

## Building

```bash
# Run all checks (fmt + clippy + test)
./scripts/build.sh --check-all

# Build release binary
cargo build --release --locked

# Bump workspace version
./scripts/version-to v0.2.0
```

## Uninstall

```bash
sudo ./scripts/uninstall.sh
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

## License

Copyright (c) 2026 rezky_nightky (oxyzenQ)

Licensed under the GNU General Public License v3.0 or later.
See [LICENSE](LICENSE) for the full text.
