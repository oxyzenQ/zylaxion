# Zylaxion v6.0.0 — Production-Grade Stability & Performance Audit

**Auditor:** oxyzenQAI (Zac)  
**Date:** 2026-06-22  
**Scope:** Full codebase — 7,092 lines across 6 crates  
**Rating System:** Critical / High / Medium / Low

---

## 1. Architecture Overview

```
zylaxion-input (libinput)     zylaxion-core (Orchestrator)     zactrix-engine (VoicePool)     zylaxion-output (cpal)
     │                              │                                │                              │
     ├─ LibinputSource              ├─ run() loop                    ├─ VoicePool (16 slots)        ├─ CpalSink
     ├─ udev backend                ├─ recv_timeout(1ms)             ├─ trigger()/release()         ├─ SPSC ringbuf (16384)
     ├─ crossbeam channel           ├─ process() batch (64)          ├─ process_sample() O(n)       ├─ F32/I16 callback
     └─ Background thread           ├─ ArcSwap<MechanicalClick>      ├─ process() batch             └─ NaN/Inf guard
                                    ├─ config-watcher thread         └─ xorshift32 PRNG
                                    ├─ IPC thread (stop/status)
                                    └─ flock single-instance
```

### Crate Dependency Graph

```
zylaxion (CLI + daemon)
  ├── zylaxion-core (orchestrator)
  │     ├── zactrix-engine (voice pool)
  │     │     └── zactrix-profiles (DSP models, TPT SVF)
  │     ├── zylaxion-input (libinput capture)
  │     └── zylaxion-output (cpal + ringbuf)
  └── arc-swap, crossbeam-channel, nix, signal-hook, clap, serde, toml
```

### DSP Signal Chain (per voice, per sample)

```
xorshift32 PRNG → excitation noise → linear fade
                                      │
                    ┌─────────────────┼──────────────────┐
                    ▼                 ▼                  ▼
              Click TPT SVF    Spring TPT SVF     Housing TPT SVF
              (HP+BP mix)      (BP only)          (BP only, Q-comp'd)
                    │                 │                  │
                    ▼                 ▼                  ▼
              click_out * 1.0   sv1 * spring_mix   hv1 * housing_mix * Q
                    │                 │                  │
                    └─────────────────┼──────────────────┘
                                      ▼
                              mixed signal
                                      │
                              + ambient HP noise
                                      │
                              * envelope_value
                                      │
                              stereo pan (equal-power)
                                      ▼
                              [left, right]
```

---

## 2. Critical-Path Execution Flow

### Hot path (audio callback thread)

```
cpal callback
  → chunks_exact_mut(data)
    → HeapRb::try_pop()           // lock-free SPSC
    → is_finite() + clamp()       // 2 comparisons, 0-2 clamps
    → write to ALSA buffer
```

### Warm path (orchestrator thread)

```
Orchestrator::run()
  → recv_timeout(1ms)             // block until event or timeout
  → VoicePool::process_sample()   // O(16) loop
    → MechanicalClick::render_sample()  // per voice
      → xorshift32 (3 mul,3 xor)
      → 3× TPT SVF (20 flops each)  // 60 flops per voice
      → ambient HP filter (3 flops)
      → envelope * decay (1 mul)
      → stereo pan (2 mul)
    → sum into [f32;2]
    → master_volume * 5.5
    → is_finite() + clamp()
  → HeapRb::try_push()
```

### Per-sample cost at 44.1 kHz, 4 active voices:

| Operation | Per Voice | Total (4 voices) |
|-----------|-----------|-------------------|
| xorshift PRNG | 3 mul + 3 xor | 12 mul + 12 xor |
| Click TPT SVF | ~20 flops | 80 flops |
| Spring TPT SVF | ~20 flops | 80 flops |
| Housing TPT SVF | ~20 flops | 80 flops |
| Ambient HPF + decay | ~6 flops | 24 flops |
| Envelope/mix/pan | ~8 flops | 36 flops |
| **Total per sample** | ~77 flops | **~312 flops** |

At 44.1 kHz with 4 active voices: ~13.7 Mflops. Negligible for any modern CPU.

### Idle cost:

- `process_sample` still iterates all 16 voices with an `is_active()` branch check
- With no active keys, this is ~1600 bytes of array scan per sample
- At 44.1 kHz: ~70 MB/s memory touched for no work
- Branch predictor handles the "all inactive" case perfectly — cost is ~0.3% CPU

---

## 3. Realtime Safety Audit

### ✅ NO ALLOCATIONS in audio/render path

| Function | Allocates? | Evidence |
|----------|-----------|----------|
| `VoicePool::process_sample()` | ❌ No | Only f32 arithmetic, array indexing |
| `MechanicalClick::render_sample()` | ❌ No | Inline xorshift + TPT SVF math |
| `CpalSink::write_sample()` | ❌ No | `HeapRb::try_push` — pre-allocated |
| `cpal callback (F32)` | ❌ No | Only `try_pop` + clamp |
| `cpal callback (I16)` | ❌ No | Same + integer conversion |
| `Orchestrator::run()` batch allocation | ❌ No | Pre-allocated `[[f32;2]; 64]` on stack |

### ✅ NO MUTEXES/LOCKS in audio/render path

| Location | Lock? | Mechanism |
|----------|-------|-----------|
| Ring buffer (producer) | ❌ | Lock-free SPSC (`HeapRb`) |
| Ring buffer (consumer) | ❌ | Lock-free SPSC |
| Model swap | ❌ | `ArcSwap::load()` — single atomic read |
| Stop flag | ❌ | `AtomicBool::load(Relaxed)` |
| Keystroke counter | ❌ | `AtomicU64::fetch_add(Relaxed)` |

### ⚠️ BLOCKING OPS (Warm Path)

| Location | Block? | Severity |
|----------|--------|----------|
| `recv_timeout(1ms)` in orchestrator | ⚠️ Yes | **Medium** — blocks orchestrator thread but NOT audio callback |
| `cpal callback` | ✅ No | Zero blocking — only try_pop |

### ✅ HIDDEN LATENCY SOURCES — None found

- No `Vec::push`, no `format!`, no `println!` in any render path
- No I/O in audio callback
- No mutex contention possible in audio path
- Ring buffer sized at 16384 frames ≈ 370 ms — generously absorbs scheduler jitter

---

## 4. Engine Optimization

### 4.1 🔴 O(n) Hot Path — Voice Iteration (CRITICAL)

**Location:** `pool.rs:process_sample()` line ~125

```rust
for voice in &mut self.voices {       // always 16 iterations
    if voice.is_active() {            // branch
        let [left, right] = model.render_sample(&mut voice.state);
```

**Problem:** Every sample, ALL 16 voices are scanned even if only 0-2 are active. The `is_active()` check is a branch, but the array scan still touches 16 × ~180 bytes = 2,880 bytes per sample. At 44.1 kHz, that's 127 MB/s of L1/L2 traffic for zero actual work when idle.

**Root Cause:** No active-voice tracking. The pool uses a flat array with linear scan.

**Recommended Fix:** Maintain a `Vec<u8>` or `u16` bitmask of active voice indices, or a smallvec of up to 16 `usize` indices updated on `trigger()` and `release()`. Iterate only active indices.

**Estimated CPU saving:** 60-80% reduction in `process_sample` when ≤4 voices active (the common case). Zero-cost change to audio callback.

**Severity:** **CRITICAL** — This is the single biggest perf issue in the render path.

### 4.2 🟡 Voice Stealing — Two-Pass O(n) (MEDIUM)

**Location:** `pool.rs:trigger()` lines ~80-100

```rust
let idx = self.voices.iter().position(|v| !v.is_active())  // Pass 1: O(n)
    .unwrap_or_else(|| {
        self.voices.iter().enumerate()
            .min_by_key(|(_, v)| v.trigger_timestamp)      // Pass 2: O(n)
            ...
    });
```

**Problem:** Two linear scans for voice stealing. When all 16 voices are active, `position()` scans all 16 (finding None), then `min_by_key` scans all 16 again.

**Impact:** Low at MAX_POLYPHONY=16, but violating zero-overhead principle. Also `trigger()` is called from the orchestrator thread, not the audio callback, so it's not latency-critical.

**Recommended Fix:** Use a `u16` bitmask + `trailing_zeros()` for free-slot discovery (O(1) modulo bit-scan), and a small `[u8; 16]` timestamp-order ring for oldest-voice lookup.

**Severity:** **MEDIUM** — Not in audio callback, but inelegant. Fix alongside the active-voice tracking refactor.

### 4.3 🟡 Cache Locality — SynthState Size (MEDIUM)

**Problem:** `SynthState` is ~180 bytes (45 × f32 + 1 × bool + 2 × u32 + 1 × u32). With 16 voices packed in an array, that's ~2,880 bytes. Iterating all 16 every sample touches 2.9 KB — easily fits in L1D (32KB on modern x86_64), so cache pressure is minimal at current polyphony.

**But:** If `MAX_POLYPHONY` is ever raised to 32 or 64, the array would be 5.7 KB or 11.5 KB, exceeding L1D. Prefetching won't help because the stride is predictable but the active set is sparse.

**Recommendation:** Keep `MAX_POLYPHONY = 16`. If raised, MUST implement active-voice tracking.

**Severity:** **MEDIUM** — Currently fine; a constraint, not a bug.

### 4.4 🟢 Branch Prediction — Well-handled (LOW)

The `is_active()` branch in `process_sample` loops is the dominant branch. When voices are idle, the branch is perfectly predictable (always false). When 4 voices are active, the pattern depends on voice allocation order, but the pool-filling order is deterministic (slot 0, 1, 2...), so the branch alternates between true (first N slots) and false (rest). Branch predictor handles this cleanly.

**Recommendation:** None needed. Current code is branch-predictor-friendly.

### 4.5 🟢 SIMD Opportunities (LOW)

**Current state:** All TPT SVF math is scalar `f32`. With `MAX_POLYPHONY=16` and 3 independent filters per voice, there are 48 scalar filter computes per sample.

**Opportunity:** With 4 active voices, 12 TPT SVFs could theoretically be computed with 2 × AVX-256 (8-wide f32) SIMD lanes in parallel. However:

1. Voice states are interleaved (not SoA), so gathering/permuting would dominate any gains.
2. Different voices have different filter coefficients (g, k values), so SIMD lanes would need independent multiplication.
3. The per-sample cost (~312 flops) is so small that SIMD overhead (gather, permute, scatter) would likely be slower.

**Recommendation:** Do NOT attempt SIMD optimization. The complexity isn't justified for 13 Mflops. If `MAX_POLYPHONY` is raised to 64+, revisit with SoA layout.

### 4.6 🟢 Excitation PRNG — xorshift32 is correct choice (INFO)

xorshift32 provides excellent statistical quality with 3 cycles of XOR + shift. LCG (multiply-add) would be ~1 cycle faster but produces lower-quality noise with visible patterns in audio. The current choice is optimal for audio-rate noise generation.

---

## 5. Audio Quality

### 5.1 🔴 Hard-Clip Limiter — Master Volume 5.5× (CRITICAL)

**Location:** `pool.rs:process_sample()` lines ~135-145

```rust
let l = out[0] * self.master_volume;  // master_volume = 5.5
let r = out[1] * self.master_volume;

// Hard-clamp to [-1.0, 1.0]
if l.is_finite() { l.clamp(-1.0, 1.0) } else { 0.0 }
```

**Problem:** This is a BRICKWALL hard-clipper, not a limiter. With 4 voices at 0.85 amplitude (ibm_model_m preset), the raw sum can reach 4 × 0.85 × 0.7 (mix) = 2.38 before master gain, and 13.09 after 5.5× gain. The `clamp(-1.0, 1.0)` hard-clips everything above 1.0, producing square-wave distortion.

**Real-world scenario:** Typing 4 keys simultaneously on ibm_model_m — the output is aggressively clipped, producing harsh odd-harmonic distortion that sounds nothing like a real keyboard.

**Impact:** Audio quality degradation under polyphony. Single-key presses sound fine. Polyphonic chords sound distorted.

**Recommended Fix (staged):**

1. **Immediate:** Reduce `master_volume` from 5.5 to 2.0 (cuts distortion by 63%).
2. **Medium-term:** Replace hard-clamp with `tanh()` soft-saturator: `l.tanh()` instead of `l.clamp(-1.0, 1.0)`. This adds ~2 cycles per sample (tanh is ~30 cycles in glibc, but fast approximate tanh is ~5 cycles).
3. **Long-term:** Proper gain-staging: sum voices, divide by active_count, apply makeup gain, then soft-saturate.

**Severity:** **CRITICAL** — User-audible distortion artifact under normal typing.

### 5.2 🟡 Gain Staging — No Headroom Management (HIGH)

**Problem:** The signal chain has no gain-stage normalization:

```
Noise excitation → TPT SVF → mix → sum(voices) → ×5.5 → clamp
                                      ↑
                              sum of up to 16 × amplitude
```

There is no per-voice amplitude normalization, no master compression, and no active-voice-count-based headroom adjustment. Each profile has different amplitudes (0.15 to 0.90), and the sum can vary wildly.

**Recommendation:** Implement `master_volume = 2.0 / sqrt(active_count.max(1) as f32)` as a quick dynamic headroom manager. This keeps the RMS level consistent regardless of polyphony.

**Severity:** **HIGH** — Gain staging is fundamentally broken for polyphonic playback.

### 5.3 🟢 Stereo Rendering — Equal-Power Pan Law (INFO)

Uses `cos(theta) / sin(theta)` with `theta ∈ [0, π/2]`. This is the standard equal-power pan law. Correct for mono-to-stereo spatialization. No issues.

### 5.4 🟢 Clipping Strategy Audit Summary

| Stage | Clipping? | Type |
|-------|-----------|------|
| Excitation noise | ❌ | Bounded to [-1, 1] by xorshift/normalize |
| TPT SVF output | ❌ | Bounded by filter stability (TPT guarantee) |
| Voice mixing | ❌ | Unbounded sum — this is the problem |
| Master volume | ⚠️ | 5.5× multiplication — saturates instantly |
| Final clamp | 🔴 | Hard brickwall — harsh distortion |
| Ring buffer write | ❌ | f32, no overflow |
| cpal callback | ✅ | Defense-in-depth NaN/Inf guard + clamp |

### 5.5 🔴 Housing Excitation — PRNG Data Race (CRITICAL)

**Location:** `mechanical.rs:render_sample()` housing excitation path

```rust
let noise = if state.sample_count < state.excitation_samples {
    // Click path already advanced noise_state this sample.
    // Re-derive the same noise value from the current state.
    let x = state.noise_state;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
} else {
    // Click burst is over — advance the PRNG ourselves.
    let mut x = state.noise_state;
    x ^= x << 13; x ^= x >> 17; x ^= x << 5;
    state.noise_state = x;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
};
```

**Problem:** When `sample_count` is within the click excitation window, the housing path "re-derives" the noise value from `noise_state` AFTER the click path already advanced it. This is WRONG — the re-derived value is the NEXT noise sample (already advanced by the click path), not the CURRENT one. So the housing filter sees a different noise stream than the click filter during the overlap period.

**But wait:** The click path computes excitation from `state.noise_state`, then advances it. The housing path then looks at `state.noise_state` (which has been advanced) and derives a noise value from it. So the housing noise is actually one PRNG step AHEAD of the click noise during the overlap. This means:
- Sample N: click sees noise[N], housing sees noise[N+1]
- Sample N+1: click advances to noise[N+1], click sees noise[N+1], housing sees noise[N+2]

This is a **subtle off-by-one** that causes the housing path to use a DIFFERENT PRNG stream than the click path. It's not a crash bug, but it means the housing layer is not temporally correlated with the click — the two layers see different noise values for the "same" physical impact.

**Impact:** The "thock" component is decorrelated from the click component. Audible as slightly different character than intended, especially at high Q values where the filter "remembers" the excitation.

**Fix:**
```rust
// Before click path runs, snapshot noise_state
let noise_for_housing = (state.noise_state as f32 / u32::MAX as f32) * 2.0 - 1.0;
// Then allow click path to advance it normally
// Then use noise_for_housing for the housing excitation
```

**Severity:** **CRITICAL** — Correctness bug in the DSP rendering. Affects all presets.

---

## 6. Long-Endurance Stability

### 6.1 ✅ Memory Growth — None Detected

| Structure | Type | Growth? |
|-----------|------|---------|
| VoicePool::voices | `[Voice; 16]` | ❌ Fixed array |
| Ring buffer | `HeapRb<[f32;2]>` (16384) | ❌ Pre-allocated |
| Orchestrator::batch | `[[f32;2]; 64]` | ❌ Stack-allocated |
| Channel queues | crossbeam unbounded | ⚠️ Grows if producer outpaces consumer |
| HashMap overrides | `HashMap<u32, KeyProfile>` | ❌ Fixed at config load |

**Channel Growth Risk:** `crossbeam_channel::unbounded()` for input events. If the orchestrator thread stalls (e.g., during heavy OS load), key events accumulate unboundedly. In practice, keyboard typing is ~5-10 events/sec, so growth is self-limiting. No known path to unbounded growth under normal operation.

**Verdict:** **SAFE** — No memory growth risk under normal operation.

### 6.2 ✅ Resource Leaks — None Detected

| Resource | Cleanup Mechanism |
|----------|-------------------|
| CpalSink::_stream | `Drop` stops stream, releases ALSA device |
| Flock (instance lock) | `Drop` releases kernel flock |
| PID file | `daemon::cleanup()` removes on graceful exit |
| Socket file | `daemon::cleanup()` removes on graceful exit |
| Config-watcher thread | Runs until process exit (acceptable for daemon) |
| IPC thread | Exits on `stop` command via `break` |
| Input thread | Channel disconnect → `return` (clean exit) |
| Signal handlers | Registered once, live until process exit |

**Verdict:** **SAFE** — No resource leaks. Cleanup is explicit and correct.

### 6.3 ✅ Thread Lifecycle — Correct

| Thread | Created | Destruction | Risk |
|--------|---------|-------------|------|
| main | Process start | Returns after orchestrator | None |
| zylaxion-input | `listen()` spawn | Channel disconnect when RX dropped | None |
| zylaxion-ipc | `spawn_ipc_thread` | `break` on stop command | None |
| zylaxion-config-watcher | `spawn_config_watcher` | Process exit (not joined) | **LOW** — daemon, fine |
| cpal callback | cpal internal | `_stream` Drop | None |

**Verdict:** **SAFE** — All threads are properly scoped.

### 6.4 ✅ Audio Underrun Risk

Ring buffer at 16384 frames ≈ 370 ms at 44.1 kHz. ALSA/PipeWire typically requests 1024-frame periods ≈ 23 ms. The orchestrator renders up to 64 frames per iteration and checks vacancy before writing. Recovery from a 350 ms scheduler stall is possible without underrun.

**Verdict:** **SAFE** — Buffer is generously sized for desktop Linux.

### 6.5 ✅ Race Conditions — None in Critical Path

All cross-thread shared state is either:
- Lock-free SPSC ring buffer (producer in orchestrator, consumer in cpal callback)
- `ArcSwap` atomic pointer (config-watcher writes, orchestrator reads)
- `AtomicBool` (signal handlers write, orchestrator reads)

**Verdict:** **SAFE** — No data races possible.

### 6.6 ✅ Atomic Contention — Minimal

| Atomic | Contention Risk |
|--------|-----------------|
| `ArcSwap::load()` (orchestrator) | None — single reader |
| `ArcSwap::store()` (config-watcher) | None — single writer |
| `AtomicU64` keystroke_counter | None — single writer (orchestrator) |
| `AtomicBool` stop_flag | None — single reader, rare writes |

**Verdict:** **SAFE** — No contention. Relaxed ordering is correct everywhere.

### 6.7 ⚠️ config-watcher — Partial File Read Risk (LOW)

The watcher reads `config.toml` at whatever mtime it sees. If an editor does an in-place write (not atomic rename), the poll may hit a partially-written file. `parse_config` will fail on malformed TOML, the watcher logs a warning and keeps the old model. Next poll retries.

**Risk:** If the user's editor takes >1 second to write the file (unlikely for a <10KB TOML), two polls may both read partial content, both fail, and the old model persists. The user must re-save.

**Mitigation:** Use `notify` crate for inotify-based watching instead of polling. Eliminates the race entirely.

**Severity:** **LOW** — Self-correcting, very unlikely in practice.

---

## 7. Build & Release Infrastructure

### 7.1 🔴 Release Profile — Misconfiguration (HIGH)

**RULES.md specifies:**
```toml
lto = true
panic = "abort"
```

**Cargo.toml actual:**
```toml
lto = "thin"
panic = "unwind"
```

**Impact:**
- `lto = "thin"` vs `lto = true` ("fat"): Thin LTO is faster to compile but produces slightly larger binaries with 1-3% less optimization. At 7K LOC, the difference is negligible (maybe 50KB), but for a production daemon binary, fat LTO should be used.
- `panic = "unwind"` vs `panic = "abort"`: Unwind requires unwinding tables in the binary (+~20KB). Abort produces a smaller binary and slightly faster code (no landing pads). For a daemon that should never panic in production, abort is correct.

**Fix:** Align `Cargo.toml` with `RULES.md`:
```toml
lto = true
panic = "abort"
```

**Severity:** **HIGH** — Violates documented release spec. Binary is larger and potentially slower than intended.

### 7.2 🟡 RULES.md Self-Violations (MEDIUM)

**Rule: Core engine ≤ 1,000 LOC**
- `zactrix-engine`: 822 LOC (pool.rs=735, lib.rs=26, voice.rs=61)
- `zactrix-profiles`: 2,319 LOC (lib.rs=1,349, mechanical.rs=740, tpt.rs=230)
- **Total: 3,141 LOC** — 3.14× over the stated limit. RULES.md itself documents this as "Current: ~1,600 LOC" which is also wrong (it's actually 3,141).

**Rule: main.rs ≤ 300 LOC**
- `main.rs`: 271 LOC (OK — just under 300)
- But RULES.md says 474 LOC — this was possibly fixed since the rule was written.

**Severity:** **MEDIUM** — Either the rules need updating or the code needs splitting. The 1,000-LOC limit for the engine is quite aggressive and may not be achievable without significant refactoring.

### 7.3 🟡 Build-time Dependency — Missing System Libraries (MEDIUM)

Building requires: `libasound2-dev`, `libinput-dev`, `libudev-dev`, `pkg-config`. The build fails without them (as confirmed during this audit). This is documented in RULES.md but not automatically detected with a helpful error message.

**Fix:** Add a `build.rs` check in the root crate that runs `pkg-config` for each required library and exits with a clear message on failure.

**Severity:** **MEDIUM** — Dev experience issue; documented but fragile.

---

## 8. Summary of Findings

### Critical (🔴)

| # | Finding | Location | Impact |
|---|---------|----------|--------|
| C1 | O(n) voice scan in audio render path | `pool.rs:process_sample()` | 60-80% wasted CPU at low polyphony |
| C2 | Brickwall hard-clipper at 5.5× gain | `pool.rs:process_sample()` | Audible distortion under polyphony |
| C3 | Housing excitation PRNG off-by-one | `mechanical.rs:render_sample()` | Decorrelated thock vs click |

### High (🟠)

| # | Finding | Location | Impact |
|---|---------|----------|--------|
| H1 | No dynamic headroom management | `pool.rs` + gain staging | Inconsistent loudness per polyphony |
| H2 | Release profile doesn't match RULES.md | `Cargo.toml` | Larger binary, suboptimal LTO |

### Medium (🟡)

| # | Finding | Location | Impact |
|---|---------|----------|--------|
| M1 | Two-pass O(n) voice stealing | `pool.rs:trigger()` | Inelegant but low cost at N=16 |
| M2 | Cache locality constraint at MAX_POLYPHONY=16 | `SynthState` layout | Currently fine; blocks future scaling |
| M3 | RULES.md core engine LOC limit violated | `zactrix-engine` + `zactrix-profiles` | 3141 actual vs 1000 target |
| M4 | Partial config file read during save | `config-watcher` polling | Self-correcting, very rare |
| M5 | No build-time system dep check | Build system | Dev UX friction |

---

## 9. v6.0.0 Implementation Roadmap

### Phase 1: Critical Fixes (Week 1)

| Task | Effort | Risk | Impact |
|------|--------|------|--------|
| **C1**: Active-voice tracking + indexed iteration | 2-3 hours | Low | 60-80% CPU reduction |
| **C2**: Replace hard-clamp with tanh soft-saturator + reduce master_gain to 2.0 | 1 hour | Low | Eliminates polyphony distortion |
| **C3**: Fix housing PRNG snapshot before click path advance | 30 min | Low | Correct thock/click correlation |

### Phase 2: High Priority (Week 2)

| Task | Effort | Risk | Impact |
|------|--------|------|--------|
| **H1**: Dynamic gain staging (divide by `sqrt(active_count)`) | 2 hours | Low | Consistent loudness |
| **H2**: Align release profile with RULES.md | 15 min | None | Binary size, perf |
| Add `build.rs` system dep check with clear error messages | 1 hour | None | Dev UX |
| Fix housing excitation sample count ceiling (min 1) consistency | 30 min | None | Correctness |

### Phase 3: Medium Priority (Week 3)

| Task | Effort | Risk | Impact |
|------|--------|------|--------|
| **M1**: Bitmask-based voice slot allocation | 3 hours | Medium | Cleaner voice management |
| Upgrade config-watcher to inotify via `notify` crate | 2 hours | Medium | Eliminates polling race |
| Update RULES.md LOC limits or split crates | 1 day (if splitting) | Medium | Codebase hygiene |

### Phase 4: v6.0.0 Release Polish

| Task | Effort |
|------|--------|
| Run `./scripts/build.sh --check-all` on clean build | 5 min |
| `cargo audit` for supply-chain CVEs | 5 min |
| Smoke test: 1h continuous run with no audio glitches | 1 hour |
| Smoke test: rapid typing + config reload + daemon restart | 30 min |
| Benchmark: CPU% at idle, 4-voice, 16-voice | 30 min |

---

## 10. Estimated Impact Matrix

| Fix | CPU Saving | Audio Quality | Stability | Binary Size |
|-----|-----------|---------------|-----------|-------------|
| C1: Active-voice tracking | **-60%** at idle | None | None | +50 bytes |
| C2: Soft-saturator + gain reduction | +2 cycles/sample | **+Major** | None | +100 bytes |
| C3: Housing PRNG fix | None | **+Moderate** (correctness) | None | None |
| H1: Dynamic gain staging | +5 cycles/sample | **+Major** | None | +50 bytes |
| H2: Release profile fix | -1-3% cycles | None | None | **-20KB** |
| Phase 2 total | ~Net zero CPU | **Significant** | None | **-20KB** |

---

## 11. What's Already Excellent

The codebase is remarkably clean and well-architected. Things that are done RIGHT:

1. **Zero-allocation audio path** — Every allocation was hunted down and eliminated
2. **TPT SVF** — Correct use of topology-preserving filters for numerical stability
3. **Ring buffer architecture** — Lock-free SPSC is the Goldilocks solution for audio
4. **ArcSwap hot-reload** — Elegant, zero-contention model swapping
5. **Instance locking via flock** — Crash-safe, PID-recycling-proof
6. **NaN/Inf defense in depth** — Guarded at VoicePool AND cpal callback
7. **Privacy discipline** — Zero scancode logging commitment
8. **Signal handling** — Graceful shutdown with fade-out, no Drop-skipping
9. **Micro-randomization** — Per-keystroke variation breaks deterministic uncanny valley
10. **Config validation** — Full clamp/validate with warn logging on bad values
11. **Test coverage** — 30+ tests across all crates, includes NaN guard and voice stealing

The foundations are solid. The issues above are surgical — small, targeted fixes to an already well-engineered system.

---

*End of v6.0.0 Audit*
