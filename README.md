<p align="center">
  <a href="https://ko-fi.com/rezky">
    <img src="https://ko-fi.com/img/githubbutton_sm.svg" alt="Support on Ko-fi">
  </a>
</p>

<h1 align="center">zylaxion</h1>

<p align="center">
  Linux-first real-time mechanical keyboard acoustic engine.<br>
  Pure procedural synthesis. Zero audio samples. Low-latency.
</p>

zylaxion transforms every keystroke into a spatially-accurate click sound
through your speakers using the kernel's evdev interface and real-time
audio via cpal / PipeWire. Every sound is generated mathematically
through TPT State Variable Filters, noise excitation, and exponential
decay envelopes — no wavetables, no sample libraries, no audio files.

**Works flawlessly on Wayland, X11, and TTY.**

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

### Quick install (system-wide)

Builds the release binary and installs it to `/usr/local/bin`:

```bash
sudo ./scripts/install.sh
```

### From source

Requires: Rust toolchain ([rustup.rs](https://rustup.rs/)),
`pkg-config`, `libasound2-dev`, `libinput-dev`, `libudev-dev`.

```bash
git clone https://github.com/oxyzenQ/zylaxion.git
cd zylaxion
cargo build --release --locked
sudo install -Dm755 target/release/zylaxion /usr/local/bin/zylaxion
sudo mkdir -p /etc/zylaxion/profiles
sudo install -m0644 profiles/*.toml /etc/zylaxion/profiles/
```

> **Note:** your user must be in the `input` group for keyboard access:
> `sudo usermod -aG input $USER && log out/in`

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
2. `/etc/zylaxion/profiles/<name>.toml` — system-wide profiles
3. `./profiles/<name>.toml` — relative to CWD (development)
4. Hardcoded default — always available

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
