# Zylaxion Future Roadmap

> **Status:** v10.0.0 released. This document is for future development.
> **Last updated:** 2026-07-01
> **Maintainer:** rezky_nightky (oxyzenQ)

---

## Locked Principles

| Principle | Detail |
|-----------|--------|
| **Linux first** | amd64 Linux (gnu binary; musl deferred — needs system lib musl ports) |
| **Low latency** | Real-time audio synthesis — must stay under 10ms |
| **Workspace architecture** | 6 crates, clean separation |
| **No bloat** | Keep dependency count reasonable |

---

## Completed

| Version | Focus | Highlights |
|---------|-------|-----------|
| v10.0.0 | Architecture Alignment | License consistency, release.yml fix, roadmap |

---

## Future Phases

### Phase 1: v10.1.0 — Polish

| Feature | Complexity |
|---------|-----------|
| Shell completions (bash/zsh/fish) | Medium |
| Man page | Medium |
| Config file validation (--testconf) | Low |
| Color output with NO_COLOR support | Low |
| Static musl binary (needs musl ports of libasound/libinput/libudev) | High |

### Phase 2: v10.2.0 — Performance

| Feature | Complexity |
|---------|-----------|
| Audio buffer pool optimization | Medium |
| Profile hot-reload (switch profiles without restart) | Medium |
| CPU affinity pinning (real-time audio thread) | Low |
| Adaptive sample rate based on output device | Medium |

### Phase 3: v11.0.0 — Ecosystem

| Feature | Complexity |
|---------|-----------|
| Custom profile import/export | Medium |
| Community profile repository | Medium |
| Prometheus metrics (latency, buffer underruns) | Low |
| D-Bus interface for desktop integration | High |

---

## Explicitly Rejected

| Feature | Why |
|---------|-----|
| ~~GUI~~ | CLI + daemon, not a GUI app |
| ~~Windows/macOS~~ | Linux input subsystem specific |
| ~~Cloud features~~ | Local audio synthesis |
