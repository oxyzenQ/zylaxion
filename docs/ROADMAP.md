# Zylaxion Future Roadmap

> **Status:** v10.0.0 released. This document is for future development.
> **Last updated:** 2026-07-01
> **Maintainer:** rezky_nightky (oxyzenQ)

---

## Locked Principles

| Principle | Detail |
|-----------|--------|
| **Linux first** | amd64 Linux (gnu binary) |
| **Low latency** | Real-time audio synthesis — must stay under 10ms |
| **Workspace architecture** | 6 crates, clean separation |
| **GPL-3.0-only** | No `or-later`, no MIT, consistent across all files |
| **No bloat** | Keep dependency count reasonable |

---

## Completed

| Version | Focus | Highlights |
|---------|-------|-----------|
| v10.0.0 | License Alignment | Full GPL-3.0-only purge, release.yml fix, Cargo.lock sync |

---

## Future Phases

### Phase 1: v10.1.0 — Polish & Stability

| Feature | Complexity |
|---------|-----------|
| Shell completions (bash/zsh/fish via clap_complete) | Low |
| Man page (clap_mangen) | Low |
| Config file validation (--testconf) | Low |
| Static musl binary (needs musl ports of libasound/libinput/libudev) | High |
| Audio buffer underrun detection + logging | Medium |
| Graceful degradation when audio device unavailable | Medium |

### Phase 2: v10.2.0 — Performance

| Feature | Complexity |
|---------|-----------|
| Audio thread CPU affinity pinning | Low |
| Adaptive sample rate based on output device | Medium |
| Profile hot-reload (switch without restart) | Medium |
| Voice stealing optimization (oldest-first priority) | Low |
| SIMD optimization for voice mixing | High |

### Phase 3: v11.0.0 — Ecosystem

| Feature | Complexity |
|---------|-----------|
| Custom profile import/export (JSON) | Medium |
| Community profile repository | Medium |
| Prometheus metrics (latency, buffer underruns, CPU) | Low |
| D-Bus interface for desktop integration | High |
| Real-time latency benchmarking tool | Medium |

---

## Explicitly Rejected

| Feature | Why |
|---------|-----|
| ~~GUI~~ | CLI + daemon, not a GUI app |
| ~~Windows/macOS~~ | Linux input subsystem specific |
| ~~Cloud features~~ | Local audio synthesis |
| ~~or-later license variant~~ | Locked to GPL-3.0-only for consistency |
