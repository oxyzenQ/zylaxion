<p align="center">
  <img src="assets/zylaxion-logo-master.png" alt="Zylaxion logo" width="260">
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

# Install (binary + config.toml go to /usr/local by default)
sudo ./scripts/install.sh
```

To install to a custom prefix:

```bash
PREFIX=/usr sudo ./scripts/install.sh
```

The installer copies the binary to `${PREFIX}/bin/zylaxion` and the
central `config.toml` to `${PREFIX}/share/zylaxion/config.toml`. It
does **not** run `cargo build` — build first, then install.

Since v3.0.0 the installer also deploys a **systemd user service** to
`~/.config/systemd/user/zylaxion.service` (when run without root) or
`/etc/systemd/user/zylaxion.service` (when run as root). To enable
auto-start on login:

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
sudo install -Dm755 zylaxion /usr/local/bin/zylaxion
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

| Preset    | Description                                      |
|-----------|--------------------------------------------------|
| technical | Crisp, loud, punchy. Cherry MX Blue click style. |
| cherryMX  | Balanced reference. General MX Blue/Brown.       |
| classic   | Deeper, resonant. Warm bucklespring tone.        |
| studio    | Softer attack, longer decay. Office-friendly.     |
| elegant   | Very soft, muffled. Low-profile keyboards.       |
| whisper   | Extremely quiet, short decay. Libraries/meetings. |

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

## License & Trademark

Copyright (c) 2026 rezky_nightky (oxyzenQ)

Licensed under the GNU General Public License v3.0 or later.
See [LICENSE](LICENSE) for the full text.

### Intellectual Property

The Zylaxion DSP architecture, TPT (Topology-Preserving Transform)
filter implementations, procedural acoustic models, and all
mathematical algorithms in the `zactrix-engine` and `zactrix-profiles`
crates are the intellectual property of `rezky_nightky (oxyzenQ)`.

Unauthorized commercial redistribution of the code or algorithms
without adhering to the GPL-3.0-or-later license is strictly
prohibited. See [docs/trademark.md](docs/trademark.md) for full
details.
