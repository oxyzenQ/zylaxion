// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

// core_tests.rs — the lib.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

#[test]
fn scancode_pan_produces_reasonable_range() {
    // Scan a wide range of scancodes and verify the pan values
    // stay within [-1, 1].  Some edge-case scancodes may land at
    // the default 0.5 → 0.0 mapping, which is fine.
    for sc in 0..200u32 {
        let pan = scancode_to_pan(sc);
        assert!(
            (-1.0..=1.0).contains(&pan),
            "pan for scancode {sc} out of range: {pan}"
        );
    }
}

#[test]
fn scancode_pan_left_keys_are_negative() {
    // Left-side keys (Q=16, A=30, Z=42) should pan left.
    assert!(scancode_to_pan(16) < 0.0, "Q should pan left");
    assert!(scancode_to_pan(30) < 0.0, "A should pan left");
}

#[test]
fn scancode_pan_right_keys_are_positive() {
    // Right-side keys (P=25, L=38, /=53) should pan right.
    assert!(scancode_to_pan(25) > 0.0, "P should pan right");
    assert!(scancode_to_pan(38) > 0.0, "L should pan right");
}

#[test]
fn scancode_pan_stereo_separation() {
    // Q (left) and P (right) should have different pan values.
    let left = scancode_to_pan(16); // Q
    let right = scancode_to_pan(25); // P
    assert_ne!(
        left, right,
        "Q and P should have different stereo positions"
    );
    assert!(left < right, "Q should be more left than P");
}

#[test]
fn scancode_pan_space_is_center() {
    // Space bar (scancode 57) should map near center.
    let pan = scancode_to_pan(57);
    assert!(
        (pan - 0.0).abs() < 0.1,
        "Space should be near center, got {pan}"
    );
}

#[test]
fn render_chunk_duration_is_sane() {
    let duration_ms = MAX_RENDER_CHUNK as f64 / 44100.0 * 1000.0;
    assert!(
        duration_ms > 0.5 && duration_ms < 10.0,
        "max render chunk duration should be 0.5–10 ms, got {duration_ms:.2} ms"
    );
}

#[test]
fn orchestrator_error_display() {
    // night-hunter-3: the exact contract — the "audio: " prefix names
    // the failing plane, and the inner audio error's own message rides
    // the chain (Display composes, it does not swallow).
    let e = OrchestratorError::Audio(crate::audio::AudioError::NoDeviceAvailable);
    let msg = e.to_string();
    assert!(msg.starts_with("audio: "), "the audio plane prefix: {msg}");
    let inner = crate::audio::AudioError::NoDeviceAvailable.to_string();
    assert!(
        msg.contains(&inner),
        "the inner error's message rides the chain: '{msg}' must contain '{inner}'"
    );
}

#[test]
fn orchestrator_error_is_error() {
    // night-hunter-3: the Error trait is not just implemented, it is
    // USEFUL — source() hands back the inner audio error (typed via
    // downcast_ref, the way any error-reporting layer would walk the
    // chain), and the trait-object coercion stays honest.
    let e = OrchestratorError::Audio(crate::audio::AudioError::NoDeviceAvailable);
    let _: &dyn std::error::Error = &e;
    let source = std::error::Error::source(&e)
        .expect("the Audio variant must expose its inner error as source");
    assert!(
        source.downcast_ref::<crate::audio::AudioError>().is_some(),
        "source() must downcast to the inner AudioError"
    );
    assert_eq!(
        source.to_string(),
        crate::audio::AudioError::NoDeviceAvailable.to_string(),
        "the source chain carries the inner error's own message"
    );
}

// ── P8: Orchestrator integration tests (v10.2.0+ — dragonzen audit) ──
//
// These tests inject a MockSink (no real audio device needed) and
// verify the full render path: trigger → render → write_sample →
// stop. They run the actual Orchestrator::run() loop in a thread,
// send keypress events through a channel, and inspect the recorded
// samples after the loop exits.

use crate::profiles::MechanicalClick;
use std::sync::Mutex;
use std::time::Instant;

// ── The Z4 witness (night-frontier-Z4) ─────────────────────────────
//
// Per-thread voluntary/nonvoluntary context switches, read from the
// thread's own /proc status file — the same mechanical surface the
// E2E battery's idle-wakeup witness uses inside the micro-VM guest.

/// Find a thread by its comm name and return (voluntary,
/// nonvoluntary) context switches. `None` = not found or unreadable —
/// the witness test fails loudly on that, it never passes silently.
fn thread_wakeups(name: &str) -> Option<(u64, u64)> {
    let tasks = std::fs::read_dir("/proc/self/task").ok()?;
    for entry in tasks.flatten() {
        let tid = entry.file_name();
        let comm =
            std::fs::read_to_string(format!("/proc/self/task/{}/comm", tid.display())).ok()?;
        if comm.trim_end() == name {
            let status =
                std::fs::read_to_string(format!("/proc/self/task/{}/status", tid.display()))
                    .ok()?;
            let mut vol = None;
            let mut nonvol = None;
            for line in status.lines() {
                if let Some(v) = line.strip_prefix("voluntary_ctxt_switches:") {
                    vol = v.trim().parse::<u64>().ok();
                }
                if let Some(v) = line.strip_prefix("nonvoluntary_ctxt_switches:") {
                    nonvol = v.trim().parse::<u64>().ok();
                }
            }
            return Some((vol?, nonvol?));
        }
    }
    None
}

/// A test-only AudioSink that records every sample written to it.
/// Uses `Arc<Mutex<Vec<...>>>` so the test can clone the Arc before
/// creating the Orchestrator and read the samples after the thread
/// joins — without needing to access the Orchestrator's private
/// `sink` field.
struct MockSink {
    samples: Arc<Mutex<Vec<[f32; 2]>>>,
    sr: u32,
}

impl MockSink {
    fn new(sr: u32) -> (Self, Arc<Mutex<Vec<[f32; 2]>>>) {
        let samples = Arc::new(Mutex::new(Vec::new()));
        let sink = Self {
            samples: Arc::clone(&samples),
            sr,
        };
        (sink, samples)
    }
}

impl AudioSink for MockSink {
    fn write_sample(&mut self, sample: [f32; 2]) {
        self.samples.lock().unwrap().push(sample);
    }
    fn producer_vacancy(&self) -> usize {
        usize::MAX // never full — orchestrator renders as fast as it can
    }
    fn sample_rate(&self) -> u32 {
        self.sr
    }
}

/// Helper: run the orchestrator in a thread with a MockSink, send
/// a single keypress event, wait briefly, then stop via channel
/// disconnect. Return the recorded samples (excluding the initial
/// pre-fill silence).
fn run_with_single_keypress(scancode: u32, wait_ms: u64) -> Vec<[f32; 2]> {
    let sr = 44_100u32;
    let (sink, samples_arc) = MockSink::new(sr);
    let mut orchestrator = Orchestrator::new_with(sink);

    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (tx, rx) = crossbeam_channel::unbounded();
    let (_stop_tx, stop_rx) = crossbeam_channel::unbounded::<ControlStop>();

    let handle = std::thread::Builder::new()
        .name("test-orchestrator".into())
        .spawn(move || {
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn test thread");

    // Send a keypress event with a monotonic timestamp.
    tx.send(KeyEvent {
        scancode,
        pressed: true,
        timestamp: 1_000_000, // 1s in us
    })
    .expect("send event");

    // Wait for the orchestrator to process the event and render.
    std::thread::sleep(Duration::from_millis(wait_ms));

    // Drop sender → channel disconnects → run() exits.
    drop(tx);

    // Wait for the thread to finish (includes fade_out_before_drop
    // which sleeps ~28ms).
    handle.join().expect("orchestrator thread join");

    // Extract samples, skipping the PREFILL_SILENCE_FRAMES (2048)
    // silence frames written at startup.
    let all_samples = samples_arc.lock().unwrap().clone();
    let skip = PREFILL_SILENCE_FRAMES.min(all_samples.len());
    all_samples[skip..].to_vec()
}

#[test]
fn trigger_produces_nonzero_audio() {
    let samples = run_with_single_keypress(30, 50);

    // After a keypress, the orchestrator should have written
    // non-zero audio samples (the click + spring + housing layers
    // all produce energy during the first few ms).
    let any_nonzero = samples
        .iter()
        .any(|[l, r]| l.abs() > 1e-8 || r.abs() > 1e-8);
    assert!(
        any_nonzero,
        "expected non-zero audio after keypress, got all silence ({} samples)",
        samples.len()
    );
}

#[test]
fn trigger_produces_decaying_amplitude() {
    // Render 100ms of audio after a keypress. The amplitude should
    // decrease over time (exponential decay envelope).
    let samples = run_with_single_keypress(30, 100);
    assert!(
        samples.len() > 100,
        "expected at least 100 samples, got {}",
        samples.len()
    );

    // Split into first third and last third. The first third
    // should have higher peak amplitude than the last third.
    let third = samples.len() / 3;
    let first_third_peak = samples[..third]
        .iter()
        .map(|[l, r]| l.abs().max(r.abs()))
        .fold(0.0f32, f32::max);
    let last_third_peak = samples[2 * third..]
        .iter()
        .map(|[l, r]| l.abs().max(r.abs()))
        .fold(0.0f32, f32::max);

    assert!(
        first_third_peak > last_third_peak,
        "amplitude should decay: first_third_peak={first_third_peak:.6}, last_third_peak={last_third_peak:.6}"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "the 200 ms bound is a real-time claim: Miri interprets every op \
              (the whole suite runs ~10x slower under it), so a wall-clock \
              evented-wake deadline cannot hold there — the message lane's \
              correctness is what Miri can still verify, and the timing pin \
              is fully live in the normal battery"
)]
fn control_stop_wakes_the_parked_loop() {
    // night-frontier-Z4: the stop lane must WAKE a parked loop. The
    // loop is idle (no voices) and parks with no timer — a message on
    // the stop lane is the wake. The margins are wide on purpose: the
    // evented wake lands in microseconds, the disconnect fallback
    // (a dropped stop sender) sits at 300 ms, so the 200 ms bound
    // proves the MESSAGE woke the loop, not the fallback — a broken
    // stop lane fails loudly here instead of hanging a CI runner.
    let sr = 44_100u32;
    let (sink, _samples) = MockSink::new(sr);
    let mut orchestrator = Orchestrator::new_with(sink);

    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (_tx, rx) = crossbeam_channel::unbounded::<KeyEvent>();
    let (stop_tx, stop_rx) = crossbeam_channel::unbounded::<ControlStop>();

    let started = Instant::now();
    let handle = std::thread::Builder::new()
        .name("test-stop".into())
        .spawn(move || {
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn");

    // Park for 50 ms, then command the stop.
    std::thread::sleep(Duration::from_millis(50));
    stop_tx.send(ControlStop).expect("send the stop");

    // The disconnect fallback arms at 300 ms (the sender's clone is
    // dropped there) so a broken message lane still terminates the
    // test — and fails the bound below, loudly.
    let fallback_tx = stop_tx.clone();
    std::thread::Builder::new()
        .spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            drop(fallback_tx);
        })
        .expect("spawn the fallback");

    let joined = handle.join();
    assert!(
        joined.is_ok(),
        "orchestrator thread should exit cleanly after the control stop"
    );
    let elapsed = started.elapsed();
    assert!(
        elapsed < Duration::from_millis(200),
        "the control stop must WAKE the parked loop (evented, ~microseconds); \
         took {elapsed:?} — the message lane is broken (the 300 ms disconnect \
         fallback answered instead)"
    );
}

#[test]
fn channel_disconnect_exits_run_cleanly() {
    let sr = 44_100u32;
    let (sink, _samples) = MockSink::new(sr);
    let mut orchestrator = Orchestrator::new_with(sink);

    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (tx, rx) = crossbeam_channel::unbounded::<KeyEvent>();
    let (_stop_tx, stop_rx) = crossbeam_channel::unbounded::<ControlStop>();

    let handle = std::thread::Builder::new()
        .name("test-disconnect".into())
        .spawn(move || {
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn");

    // Drop the sender — the parked loop's event arm sees the
    // disconnect as a ready op, causing run() to exit.
    std::thread::sleep(Duration::from_millis(20));
    drop(tx);

    let joined = handle.join();
    assert!(
        joined.is_ok(),
        "orchestrator thread should exit cleanly after channel disconnect"
    );
}

#[test]
fn hot_reload_model_swap_mid_render() {
    // Trigger a keypress with one model, swap the model mid-render,
    // trigger another keypress. Both should produce audio — the
    // second should use the new model.
    let sr = 44_100u32;
    let (sink, samples_arc) = MockSink::new(sr);
    let mut orchestrator = Orchestrator::new_with(sink);

    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let model_for_thread = Arc::clone(&model);
    let (tx, rx) = crossbeam_channel::unbounded();
    let (_stop_tx, stop_rx) = crossbeam_channel::unbounded::<ControlStop>();

    let handle = std::thread::Builder::new()
        .name("test-hotreload".into())
        .spawn(move || {
            orchestrator.run(&model_for_thread, &rx, &stop_rx);
        })
        .expect("spawn");

    // First keypress with the original model.
    tx.send(KeyEvent {
        scancode: 30,
        pressed: true,
        timestamp: 1_000_000,
    })
    .expect("send 1");
    std::thread::sleep(Duration::from_millis(20));

    // Swap in a new model (different pitch drift seed will produce
    // a different waveform). ArcSwap::store works through &self.
    model.store(Arc::new(MechanicalClick::new(sr)));
    std::thread::sleep(Duration::from_millis(10));

    // Second keypress with the new model.
    tx.send(KeyEvent {
        scancode: 31,
        pressed: true,
        timestamp: 2_000_000,
    })
    .expect("send 2");
    std::thread::sleep(Duration::from_millis(30));

    drop(tx);
    handle.join().expect("join");

    let samples = samples_arc.lock().unwrap().clone();
    let skip = PREFILL_SILENCE_FRAMES.min(samples.len());
    let audio = &samples[skip..];

    // Should have non-zero audio from both keypresses.
    let any_nonzero = audio.iter().any(|[l, r]| l.abs() > 1e-8 || r.abs() > 1e-8);
    assert!(
        any_nonzero,
        "expected non-zero audio after hot-reload + 2 keypresses"
    );
}

#[test]
fn master_volume_affects_output_amplitude() {
    // Two runs with different master volumes. The higher-volume
    // run should produce louder output (higher peak).
    let sr = 44_100u32;

    // Low volume run.
    let (sink_lo, samples_lo) = MockSink::new(sr);
    let mut orch_lo = Orchestrator::new_with_volume(sink_lo, 1.0);
    let model_lo = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (tx_lo, rx_lo) = crossbeam_channel::unbounded();
    let (_stop_tx_lo, stop_rx_lo) = crossbeam_channel::unbounded::<ControlStop>();
    let handle_lo = std::thread::spawn(move || {
        orch_lo.run(&model_lo, &rx_lo, &stop_rx_lo);
    });
    tx_lo
        .send(KeyEvent {
            scancode: 30,
            pressed: true,
            timestamp: 1_000_000,
        })
        .ok();
    std::thread::sleep(Duration::from_millis(50));
    drop(tx_lo);
    handle_lo.join().unwrap();

    // High volume run.
    let (sink_hi, samples_hi) = MockSink::new(sr);
    let mut orch_hi = Orchestrator::new_with_volume(sink_hi, 10.0);
    let model_hi = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (tx_hi, rx_hi) = crossbeam_channel::unbounded();
    let (_stop_tx_hi, stop_rx_hi) = crossbeam_channel::unbounded::<ControlStop>();
    let handle_hi = std::thread::spawn(move || {
        orch_hi.run(&model_hi, &rx_hi, &stop_rx_hi);
    });
    tx_hi
        .send(KeyEvent {
            scancode: 30,
            pressed: true,
            timestamp: 1_000_000,
        })
        .ok();
    std::thread::sleep(Duration::from_millis(50));
    drop(tx_hi);
    handle_hi.join().unwrap();

    // Compare total energy (sum of squares). Peak doesn't work
    // because the hard clamp in process_sample saturates both to
    // 1.0. Energy is a better proxy for perceived loudness —
    // higher volume means more samples are near the clamp ceiling,
    // so total energy is higher.
    let skip = PREFILL_SILENCE_FRAMES;
    let energy_lo: f32 = samples_lo.lock().unwrap()[skip..]
        .iter()
        .map(|[l, r]| l * l + r * r)
        .sum();
    let energy_hi: f32 = samples_hi.lock().unwrap()[skip..]
        .iter()
        .map(|[l, r]| l * l + r * r)
        .sum();

    assert!(
        energy_hi > energy_lo,
        "higher master volume should produce more energy: energy_lo={energy_lo:.4}, energy_hi={energy_hi:.4}"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "the witness reads /proc, which Miri's isolated runs do not expose"
)]
fn idle_park_emits_zero_wakeups() {
    // night-frontier-Z4's core claim, witnessed mechanically: the
    // idle render loop is a PARKED thread — zero voluntary and zero
    // nonvoluntary context switches over a real 2 s silent window.
    // The pre-Z4 loop's 100 ms idle timeout produced ~20 voluntary
    // switches per second in this exact regime; the parked loop
    // produces none, because nothing wakes it but a channel op.
    let sr = 44_100u32;
    let (sink, _samples) = MockSink::new(sr);
    let mut orchestrator = Orchestrator::new_with(sink);

    let model = Arc::new(ArcSwap::from_pointee(MechanicalClick::new(sr)));
    let (tx, rx) = crossbeam_channel::unbounded::<KeyEvent>();
    let (stop_tx, stop_rx) = crossbeam_channel::unbounded::<ControlStop>();

    let handle = std::thread::Builder::new()
        .name("test-park".into())
        .spawn(move || {
            orchestrator.run(&model, &rx, &stop_rx);
        })
        .expect("spawn");

    // Let the loop reach its park, then witness a 2 s silent window.
    std::thread::sleep(Duration::from_millis(200));
    let before = thread_wakeups("test-park")
        .expect("the parked loop thread must be findable in /proc/self/task");
    std::thread::sleep(Duration::from_millis(2000));
    let after = thread_wakeups("test-park").expect("the parked loop thread must still be findable");

    drop(stop_tx); // a dropped stop sender is a ready op — the park wakes
    drop(tx);
    let _ = handle.join();

    assert_eq!(
        after.0 - before.0,
        0,
        "ZERO voluntary context switches expected while idle parked: \
         a timer wake hides in the idle wait"
    );
    assert_eq!(
        after.1 - before.1,
        0,
        "ZERO nonvoluntary context switches expected: a parked thread never runs"
    );
}
