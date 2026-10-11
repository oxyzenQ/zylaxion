// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The config watcher: hot-reload of preset.tuning on file change.
//!
//! Split from daemon.rs in night-improve-18 (the no-mercy LOC law):
//! the watch loop and its mtime/size probes are one self-contained
//! concern. `pub(super)` so the parent daemon module is the only
//! consumer.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, SystemTime};

use arc_swap::ArcSwap;
use zactrix_profiles::MechanicalClick;

/// Polling interval for the config-file watcher.
///
/// 1 second is the sweet spot: short enough that users perceive config
/// edits as "instant" after save, long enough that the watcher thread
/// consumes negligible CPU (one `stat()` per second).
const CONFIG_WATCH_INTERVAL: Duration = Duration::from_secs(1);

// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

/// Spawn a background thread that watches `config.toml` for changes
/// and atomically swaps the acoustic model when the file is modified.
///
/// # Behaviour
///
/// - Polls every [`CONFIG_WATCH_INTERVAL`] (1 second).
/// - On every poll, re-runs the search-path resolution via
///   [`crate::session::config::find_config_path`]. This catches the case where
///   a higher-priority config file appears while the daemon is
///   running (e.g. user creates `~/.config/zylaxion/config.toml`
///   while the daemon was watching `/usr/local/share/zylaxion/...`).
///   When the resolved path changes, the watcher logs the switch and
///   loads the new file immediately.
/// - For the currently watched path, polls its modification time
///   (mtime) via `std::fs::metadata`. When the mtime advances, the
///   thread re-reads the file via [`config::reload_preset`].
///   **The `--preset` CLI flag is ignored on reload** — the watcher
///   always reads `preset.tuning` from the freshly-saved file.
/// - On any error (parse failure, preset not found), the thread logs
///   a `warn!` and keeps the old model. The user can fix the TOML and
///   save again to retry.
/// - If `config_path` is `None` at startup (hardcoded default, no
///   file found anywhere), the thread does NOT exit — it keeps
///   polling so it picks up a config file the moment one appears in
///   any search path.
///
/// # `initial_preset` is for logging only
///
/// The `initial_preset` parameter is used solely for the startup log
/// message ("watching ... initial preset: X"). It is NOT passed to
/// `reload_preset` — the watcher always defers to `preset.tuning` on
/// file change.
///
/// # No blocking of the audio callback
///
/// `ArcSwap::store()` is a single atomic pointer swap. The cpal audio
/// callback never touches the ArcSwap — it only reads the cached
/// `KeyProfile` snapshot that each voice captured at trigger time.
///
/// # Graceful shutdown (v10.2.0+ — dragonzen audit S1)
///
/// The watcher accepts a clone of the orchestrator's `stop_flag`. It
/// checks the flag at the top of every poll iteration and exits
/// cleanly when the flag is set. Previously the watcher had no
/// shutdown signal — it leaked at process exit, holding a clone of
/// the `Arc<ArcSwap<MechanicalClick>>` and preventing clean Drop of
/// the model.
pub(super) fn spawn_config_watcher(
    model: Arc<ArcSwap<MechanicalClick>>,
    config_path: Option<std::path::PathBuf>,
    initial_preset: String,
    sample_rate: Arc<std::sync::atomic::AtomicU32>,
    stop_flag: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("zylaxion-config-watcher".into())
        .spawn(move || {
            // `current_path` is the file the watcher is currently
            // watching. It can change mid-run if a higher-priority
            // file appears in the search path (see `find_config_path`
            // for the resolution order).
            let mut current_path: Option<std::path::PathBuf> = config_path.clone();

            // `last_mtime` is the mtime of `current_path` as observed
            // on the previous poll. Used to detect in-place edits to
            // the currently watched file. Reset to `None` whenever
            // `current_path` changes so the new file is loaded on the
            // next iteration regardless of its mtime.
            let mut last_mtime: Option<SystemTime> = current_mtime_of(&current_path);
            // v10.2.0 (dragonzen audit P3): track file size alongside
            // mtime to suppress spurious reloads triggered by chmod/
            // chown/utime. Previously `chmod` advanced the mtime
            // without changing content, and the watcher re-read +
            // re-built the model unnecessarily. Now we require BOTH
            // mtime advance AND size change (or size unknown on the
            // previous poll) before reloading.
            let mut last_size: Option<u64> = current_size_of(&current_path);

            log::info!(
                "config-watcher: initial path = {} (initial preset: {}, poll interval: {:?})",
                path_display(&current_path),
                initial_preset,
                CONFIG_WATCH_INTERVAL
            );

            loop {
                // v10.2.0 (S1): check stop_flag at the top of every
                // iteration so the watcher exits cleanly when the
                // orchestrator is shutting down.
                if stop_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    log::info!("config-watcher exiting (stop flag set)");
                    return;
                }

                std::thread::sleep(CONFIG_WATCH_INTERVAL);

                // ── 1. Re-evaluate the search path ────────────────────
                // This is the v4.1.0 addition: detect when a higher-
                // priority config file appears while the daemon is
                // already running. Without this, the watcher would be
                // locked to whatever path was resolved at startup.
                let resolved = crate::session::config::find_config_path();

                if resolved != current_path {
                    log::info!(
                        "config-watcher: config path changed: switching from {} to {}",
                        path_display(&current_path),
                        path_display(&resolved)
                    );
                    current_path = resolved;
                    // Force a reload on the next step by clearing the
                    // mtime AND size baselines. The new file might
                    // have any mtime/size, so we cannot rely on the
                    // previous values (v10.2.0 — P3: also reset size).
                    last_mtime = None;
                    last_size = None;

                    // If the new path is None (all config files
                    // vanished), keep the old model and continue
                    // polling — the user might be in the middle of
                    // moving files around.
                    if current_path.is_none() {
                        log::warn!(
                            "config-watcher: no config.toml found in any search path — \
                             keeping previous model, will retry next poll"
                        );
                        continue;
                    }
                }

                // ── 2. Check the currently-watched file's mtime ──────
                let Some(path) = current_path.as_ref() else {
                    // No file to watch yet (initial hardcoded-default
                    // case). The search-path re-evaluation above will
                    // pick up a file the moment one appears.
                    continue;
                };

                let now_mtime = match current_mtime(path) {
                    Some(t) => t,
                    None => {
                        // The file disappeared between the search-path
                        // check and now (race). The next poll's
                        // search-path re-evaluation will handle the
                        // fallback. Just reset the baselines so we
                        // don't miss a recreation (v10.2.0 — P3: also
                        // reset size).
                        last_mtime = None;
                        last_size = None;
                        continue;
                    }
                };

                // night-improve-4 (edition 2024): let-chains collapse
                // the nested guard clippy flagged as collapsible_if.
                if let Some(prev) = last_mtime
                    && now_mtime <= prev
                {
                    // mtime hasn't advanced — nothing to do.
                    continue;
                }

                // v10.2.0 (dragonzen audit P3): also require a size
                // change (or unknown previous size) before reloading.
                // `chmod`, `chown`, and `utime` advance the mtime
                // without changing content — a backup tool that
                // touches file mtimes (e.g. `rsync --archive`) would
                // trigger spurious reloads without this guard.
                let now_size = current_size(path);
                let size_changed = match (last_size, now_size) {
                    (Some(prev), Some(now)) => prev != now,
                    _ => true, // Unknown previous or current size — be safe.
                };
                if !size_changed {
                    // mtime advanced but size is identical — likely a
                    // chmod/utime. Skip the reload but update the
                    // baseline so we don't keep checking.
                    last_mtime = Some(now_mtime);
                    last_size = now_size;
                    log::debug!(
                        "config-watcher: {} mtime advanced but size unchanged — skipping reload (likely chmod/utime)",
                        path.display()
                    );
                    continue;
                }

                log::info!("config-watcher: {} changed, reloading", path.display());
                last_mtime = Some(now_mtime);
                last_size = now_size;

                // reload_preset reads preset.tuning from the file — the
                // initial --preset CLI flag is intentionally NOT passed
                // here so that file edits always take precedence.
                match crate::session::config::reload_preset(path, None) {
                    Ok((profiles, active, master)) => {
                        // v10.2.0 (B13): read the current sample_rate
                        // from the shared atomic instead of using the
                        // stale captured value. If the audio device's
                        // rate ever changes mid-run, the watcher picks
                        // up the new rate automatically.
                        let sr = sample_rate.load(std::sync::atomic::Ordering::Relaxed);
                        let new_model =
                            MechanicalClick::with_overrides(profiles, sr);
                        model.store(Arc::new(new_model));
                        log::info!("config-watcher: reloaded preset '{active}' successfully (sample_rate={sr})");
                        // v10.2.0 (P1): master volume is part of the
                        // config but lives in the VoicePool, not the
                        // AcousticModel. Hot-reload of master volume is
                        // not yet wired — the user must restart the
                        // daemon for [master].volume changes to take
                        // effect. Log the loaded value for visibility.
                        log::debug!(
                            "config-watcher: master volume from reload = {}× (hot-reload not yet supported — restart to apply)",
                            master.volume
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "config-watcher: failed to reload {} — keeping previous config. Error: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        })
        .expect("failed to spawn config-watcher thread")
}

/// Get the modification time of a path (wrapped in `Option`) as a
/// `SystemTime`. Returns `None` if the path is `None` (no file
/// configured) or if the metadata cannot be read.
fn current_mtime_of(path: &Option<std::path::PathBuf>) -> Option<SystemTime> {
    path.as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .and_then(|m| m.modified().ok())
}

/// Format an `Option<PathBuf>` for display in log messages.
/// `None` becomes `"<hardcoded default>"` so the log line is
/// unambiguous: it means no config file was found anywhere and the
/// daemon is running with the built-in `KeyProfile::default()`.
fn path_display(path: &Option<std::path::PathBuf>) -> String {
    match path {
        Some(p) => p.display().to_string(),
        None => "<hardcoded default>".to_string(),
    }
}

/// Get the modification time of a path as a `SystemTime`, or `None` if
/// the file does not exist or its mtime cannot be read.
fn current_mtime(path: &std::path::Path) -> Option<SystemTime> {
    std::fs::metadata(path).ok().and_then(|m| m.modified().ok())
}

/// Get the file size of an optional path, or `None` if the path is
/// `None` or the metadata cannot be read (v10.2.0+ — dragonzen audit
/// P3). Used alongside `current_mtime_of` to detect chmod-triggered
/// spurious mtime advances.
fn current_size_of(path: &Option<std::path::PathBuf>) -> Option<u64> {
    path.as_ref()
        .and_then(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
}

/// Get the file size of a path, or `None` if the file does not exist
/// or its metadata cannot be read (v10.2.0+ — dragonzen audit P3).
fn current_size(path: &std::path::Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|m| m.len())
}
