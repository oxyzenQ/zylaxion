<!-- Copyright (C) 2026 rezky_nightky -->
<!-- SPDX-License-Identifier: GPL-3.0-only -->

# zylaxion Project Rules

Absolute source of truth for maintaining consistency, efficiency, and quality.

## Architecture & Code Quality

- **LOC law (night-improve-18, the no-mercy era):** no single `.rs`
  file anywhere in the tree may exceed 600 lines — no marker, no
  mercy. `scripts/gates/check-loc.sh` walks the whole git index and
  fails the gate on any over-cap file; over the cap means SPLIT, the
  same hour, a pure move with gates green in between (the former
  `// LOC_EXEMPT:` debt lane is retired — all six pre-gate files were
  split). The same gate enforces the layout law: the package's src/
  root holds exactly its entry file (`main.rs` for the binary,
  `lib.rs` for a library), every other module lives in its subsystem
  directory (`dir/mod.rs` style), and every `*.rs` test file lives
  under the package's `test/` tree (the Pattern C `#[path]`
  wiring, one theme per file) under the same cap.
- **Modular `main.rs`:** `src/main.rs` MUST stay within 100–300 LOC. Bootstrap and wiring only. Logic goes into specific
  modules (the zelynic foundation shape, night-improve-14).
- **Release Profile:** `[profile.release]` uses `lto = "thin"`, `codegen-units = 1`,
  `opt-level = 3`, `strip = true`, `panic = "unwind"`.

## Kernel Surfaces (night-improve-25)

Kernel surfaces (tracepoints, symbols, BTF names) are claims. Verify
against mainline before coding to them. Cite the file and line, or do
not claim it. The `input_event` tracepoint shipped as load-bearing
fiction for a full revision because a research doc's fallback note
was copied forward without checking `include/trace/events/` — the
exported `input_event()` function was the approved plan all along,
and a five-minute source check would have caught the drift before
any code existed. The same law applies to kprobe blacklist
assumptions, BTF symbol availability, and every `CONFIG_*` fact a
refusal message states as certain.

## Version & Update Command

All `-V` / `--version` output MUST follow this exact format:

```
Version: v0.1.0
Build: linux-amd64 (11114e7)
Copyright: (c) 2026 rezky_nightky (oxyzenQ)
License: GPL-3.0-only
Source: https://github.com/oxyzenQ/zylaxion
```

A `--check-update` subcommand MUST be implemented to check the latest
upstream GitHub release.

## Local Tooling

- **`scripts/build.sh --check-all`:** Local CI gatekeeper. Runs four
  checks sequentially, exiting on first failure:
  1. `cargo fmt --check` — formatting must be clean.
  2. `cargo clippy -- -D warnings` — zero warnings allowed.
  3. `cargo test` — all unit + doc tests must pass.
  4. `cargo audit` (v5.0.0+) — supply-chain CVE scan. Optional: if
     `cargo-audit` is not installed, the script prints a warning and
     continues instead of failing. Install with `cargo install
     cargo-audit` to enable the audit gate.
- **`scripts/dev/version-to.sh vMAJOR.MINOR.PATCH`:** Single-source-of-truth version bumper.
  Patches all `Cargo.toml` files. Zero tolerance for manual version edits.

## Build Prerequisites

zylaxion plays audio through `cpal`, which talks to the OS audio server
(PipeWire / PulseAudio / ALSA) via the ALSA backend. Only the ALSA
development headers are required at build time; no PipeWire native
headers are needed.

CI installs them in `.github/workflows/ci.yml` via `apt-get install`:

- `pkg-config` — needed to locate `alsa.pc`.
- `libasound2-dev` — ALSA C headers (consumed by `cpal`'s ALSA backend).
- night-improve-23: the `libinput-dev`/`libudev-dev` rows are gone — the
  input lane is the kernel-side eBPF tap, which needs no input
  libraries, and the BPF object ships embedded (developing `ebpf/`
  additionally needs the dated nightly + bpf-linker:
  `./scripts/dev/bootstrap-ebpf.sh`).

A CI runner missing any of these will fail at `cargo clippy` with
`pkg-config` exiting non-zero.

> **History note:** v2.0.0 briefly introduced native PipeWire integration
> via `pipewire-rs` (which pulled in `bindgen`, `libclang-dev`,
> `libpipewire-0.3-dev`, `libspa-0.2-dev`). This was reverted in v3.0.0
> because the `pipewire-rs` crate is unmaintained and breaks on PipeWire
> 1.0+ systems. The ALSA bridge used since v1.0.x has near-zero overhead
> and is rock-solid.

## Kernel-side Boot Oneshot (night-improve-23)

- `assets/zylaxion-attach.service` is a systemd **system** ONESHOT unit
  (`Type=oneshot`, `RemainAfterExit=no`) — NOT a daemon: it runs
  `zylaxion attach`, exits, and stays dead. What remains resident is
  the kernel side only (one passive BPF program + pinned maps).
- `scripts/package/install.sh` deploys it to
  `/etc/systemd/system/zylaxion-attach.service` and enables it; bpffs
  pins do not survive a reboot, so the oneshot replants the tap at
  every boot. The install posture is system-only (night-master-2):
  binary to `/usr/bin`, oneshot to `/etc/systemd/system`, no flags.
- `assets/zylaxion-handover.path` + `zylaxion-handover.service` are
  the login hand-over (night-dinner-19): the boot oneshot runs before
  anyone logs in, so the pins it plants are root-owned; the path unit
  watches `/run/user/*` and re-runs `zylaxion attach` at the first
  desktop login, which hands the pins to that user and relaxes the
  bpffs front door. Both installers deploy and enable the pair; the
  uninstaller removes it.
- `scripts/package/install-on-my-machine.sh` is the CPU-tuned variant
  of the same posture (amd64 v3/v4 detection, parity-checked aliases).
- A hand-planted tap stays available without any installer:
  `sudo zylaxion attach` (which also hands the pin ownership to the
  invoking user — and, since night-dinner-19, heals an
  already-attached root-owned pin set in place, no detach needed).
- The user service of the daemon era is gone with the daemon — there is
  nothing to supervise anymore.

## Security & Privacy (v3.1.0+)

zylaxion has a **zero-leakage** posture. Hard rules:

- **Never log scancodes.** The `KeyEvent` struct (in `src/core/`)
  carries a raw hardware scancode that can be mapped back to typed
  text via the user's keyboard layout. Logging it — even at `debug`
  or `trace` level — would create a side-channel leak exploitable by
  anyone who can read the player's stderr: the terminal that spawned
  it, a shell scrollback, or any log capture that stderr reaches.
  - **Forbidden**: `log::debug!("scancode: {}", event.scancode)`,
    `println!("key: {:?}", event)`, `eprintln!("got {}", event.scancode)`.
  - **Allowed**: `log::debug!("input event received")` (generic,
    no scancode, no key identity).
- **The control plane has no protocol.** night-improve-23 deleted the
  IPC socket (and its `0o600` law died with it — there is no socket).
  The pinned BPF maps are the only control surface; bpffs file
  permissions decide who can write them, and the attach hands the
  pins to the invoking sudo user.
- **No inbound network.** zylaxion opens no TCP listener, no HTTP
  server, no gRPC port, no Unix socket.
- **No outbound telemetry.** The only network call is the explicit
  `zylaxion --check-update` subcommand, which fetches one GitHub
  release tag and exits. There is no background phone-home, no
  analytics, no crash-reporter.
- **No on-disk state.** zylaxion writes no files anywhere — no pid
  file, no socket (the IPC socket was deleted with the daemon in
  night-improve-23; `docs/GLOSSARY.md`'s socket entry is history),
  no history file, no debug log, no profile dump. The player's pid
  lives in the pinned control map's `PLAYER_PID` slot (the "pidfile
  in the kernel"), and the single-instance and liveness surfaces read
  that slot — the disk inventory is empty by construction.

Audit verification (run before every release):

```bash
# Should print zero matches in production paths:
grep -rn 'scancode' src/ examples/ --include='*.rs' | \
    grep -E 'log::|println!|eprintln!' | \
    grep -v 'examples/'
```

## CI/CD (GitHub Actions)

- **CI paths filter (code-push workflows):** the shared surface globs
  — `**/*.rs`, `**/*.toml`, `**/Cargo.lock`, `.cargo/**`, `ebpf/**`,
  `scripts/**` — plus the workflow's own file (night-badas-2, the
  zelynic boost-10 one-spelling law). `*.md`, `*.txt` and `docs/`
  match nothing: a docs-only commit never buys a code lane.
- **No detector job:** the former `changes` job (a full
  `fetch-depth: 0` clone plus a git-diff regex re-implementing
  GitHub's native path filter) is gone (night-badas-6, the zelynic
  NIGHT-improve-13 lineage). The `paths:` filter IS the single
  trigger gate — no depth-0 clone is spent detecting what GitHub
  already knows.
- **Gate-keepers CI (night-badas-5):** `Dragon Guard - Gate-keepers`
  (gate-keepers.yml) runs on EVERY push and PR, UNFILTERED — the
  wholesale non-code gates (permissions, headers, LOC caps, language,
  emoji, actions pins, release parity, name-case) plus the workflow
  lint quad (actionlint, yamllint, codespell) can never be skipped by
  a docs-only change, the exact class of commit they police. Zero
  build minutes: bash, git, python3 and four pinned lint tools.
- **Scanner rule:** a scanner workflow carries the surface it scans
  (the zelynic NIGHT-improve-11 law) — `Dragon Guard - Security
  Audit` (audit.yml, cargo-audit `--deny warnings`) triggers on
  `**/*.toml` + `**/Cargo.lock` + its own file, with the weekly
  Monday 07:00 UTC clock as the between-pushes advisory-DB watch.
  It is the canonical CVE verdict; the local gatekeeper skips
  cargo-audit per policy.
- **Node.js:** `FORCE_JAVASCRIPT_ACTIONS_TO_NODE24=true` in all workflow env.
- **Dependabot:** REMOVED. Do not use Dependabot.
- **Dependency updates:** Custom `maintenance.yml` workflow auto-updates deps
  and commits directly to `main`. NO PRs. NO branch spam.
- **Maintenance schedule:** Weekly Monday 07:00 UTC.
- **Bot identity:** Commits as `github-actions[bot]`.
- **Workflow titles:** the `Dragon Guard - <lane>` family, exactly as
  in the zelynic reference (CI, Supermassive, Miri, Release,
  Dependency Maintenance, crates.io publish, Gate-keepers, Security
  Audit) — one family, one grep surface (docs/GLOSSARY.md,
  "Dragon Guard").
- **Supermassive E2E leg names:** the zelynic low/best specs shape —
  `supermassive test - low specs (the 5.15 documented floor)` and
  `supermassive test - best specs (the archive's head)` (night-badas-5).
  No musl twins: the musl lane is the documented dormant lane
  (.cargo/config.toml header).
- **Linting:** Run `actionlint` and `yamllint` on `.github/workflows/*`.

## Branding & Metadata

- **Author:** `rezky_nightky (oxyzenQ)`. No casing inconsistencies.
- **Repository:** `github.com/oxyzenQ/zylaxion` (capital Q).
- **Contact:** `with dot rezky at gmail dot com`.
- **Project name:** `zylaxion` (lowercase). Never capitalized in any
  context — machine-enforced by `scripts/gates/check-name-case.py`.
- **Badges:** ONLY Ko-fi (`ko-fi/rezky`).
- **License:** GPL-3.0-only.
- **Trademark:** See `TRADEMARK.md` (the repo root — the
  `docs/trademark.md` path this row used to name does not exist).
- **File headers:** All `.rs`, `.sh`, `.yml` files MUST carry:
  ```
  Copyright (C) 2026 rezky_nightky
  SPDX-License-Identifier: GPL-3.0-only
  ```

## Git & Repo Hygiene

- `.gitignore`: Keep lean. Ignore `worklog.md`, `codex/`, `agent/`, `agent-ctx/`,
  and other AI tool directories.
- No tracked AI artifacts.

## Preset System (zylaxion-specific, night-improve-22 the no-config law)

- All acoustic presets are compile-time data in `src/profiles/registry.rs`:
  24 hand-tuned presets, type-checked by rustc, pinned in-range by the
  registry test tree (each value must survive `validate_and_clamp` with
  zero drift). There is no config file, no search path, no watcher.
- Active preset resolution lives in `src/session/preset.rs`:
  `--preset <name>` on the CLI → `ZYLAXION_PRESET` in the environment →
  the compiled default (`technical`). The master output volume follows
  the same law via `ZYLAXION_VOLUME` (default 5.5, invalid values are
  hard errors — no silent fallback anywhere).
- A preset is chosen when the sound starts; while playing,
  `zylaxion preset --switch <name>` swaps it live (one control-map
  write). The former 1-second mtime poller and the `testconf`
  validator died with the config file — a validator for compile-time
  data would validate nothing.
- Unknown preset names produce the canonical error with the full
  catalog plus the did-you-mean tip (the suggestion engine, edit
  distance 2).
- DSP parameters flow: `registry` → `session::preset::resolve()` →
  `MechanicalClick::with_profile()` → `VoicePool::trigger()` →
  `init_state()`. All TPT filter coefficients are set from profile
  values — this chain must never be broken.
