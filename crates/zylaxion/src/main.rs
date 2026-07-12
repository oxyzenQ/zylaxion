// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! **Zylaxion** — real-time mechanical keyboard acoustic synthesizer for Linux.
//!
//! Transforms every keystroke into a spatially-accurate click sound
//! through your speakers, using the kernel's evdev interface and
//! low-latency audio via cpal / PipeWire.
//!
//! # Architecture
//!
//! - [`cli`]       — clap argument definitions & masterclass version format
//! - [`config`]    — filesystem search for the central `config.toml`
//! - [`commands`]  — subcommand handlers (`start`, `daemon`, `stop`, `doctor`, …)
//! - [`daemon`]    — POSIX daemonization, PID file, signal handling, IPC

use clap::Parser;

mod cli;
mod commands;
mod config;
mod daemon;
mod error_format;
mod instance_lock;
mod pathguard;
mod signals;

fn main() {
    // v10.2.0 (dragonzen audit I6): set RUST_LOG as the very first
    // thing, BEFORE `Cli::parse()`. The previous order (parse first,
    // then set_var) worked because clap is single-threaded today,
    // but that's not a stable contract — clap's help/version flags
    // could spawn threads in the future. Doing it first eliminates
    // the race window entirely.
    //
    // We peek argv directly (no clap) for `--verbose` so we don't
    // depend on parser internals. A pre-existing RUST_LOG env var
    // takes precedence over the `--verbose` heuristic — power users
    // who set RUST_LOG=trace manually keep their setting.
    let verbose = std::env::args().any(|a| a == "--verbose" || a == "-v");
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", if verbose { "debug" } else { "info" });
    }

    // v10.2.0 (dragonzen audit S3): install a panic hook that logs
    // the panic location + payload before the default handler prints
    // to stderr. This is critical for the cpal audio callback path:
    // if a future code change introduces an `unwrap` on `None` inside
    // the callback, cpal catches the unwind and silently drops the
    // stream — the daemon stays alive but audio goes silent. Without
    // a panic hook, the journal would have no record of WHY the
    // stream died. With the hook, `journalctl --user -u zylaxion`
    // shows the panic message + backtrace, making the silent-audio
    // failure mode debuggable.
    //
    // The hook delegates to the previous handler (chain pattern) so
    // existing panic output to stderr is preserved.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".to_string());
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "<non-string panic payload>".to_string());
        log::error!("panic at {location}: {payload}");
        // Chain to the previous hook so stderr output is preserved
        // for foreground mode (where journald captures stderr).
        prev_hook(info);
    }));

    // Early-exit flags — intercepted BEFORE `Cli::parse()` to bypass Clap's
    // subcommand requirement. This is the bulletproof way to handle early-exit
    // flags without fighting the parser: `zylaxion --check-update` (with no
    // subcommand) would otherwise error out before the flag is ever processed.
    if std::env::args().any(|a| a == "--check-update") {
        commands::update::run_check_update();
        std::process::exit(0);
    }

    let cli = cli::Cli::parse();

    match cli.command {
        cli::Commands::Start { preset } => commands::daemon::cmd_start(preset),
        cli::Commands::Daemon { preset, foreground } => {
            commands::daemon::cmd_daemon(preset, foreground)
        }
        cli::Commands::Stop => commands::daemon::cmd_stop(),
        cli::Commands::Status => daemon::cmd_status(),
        cli::Commands::Doctor => commands::info::cmd_doctor(),
        cli::Commands::Testconf { file } => commands::info::cmd_testconf(file.as_deref()),
        cli::Commands::ListPresets => commands::info::cmd_list_presets(),
        cli::Commands::ListBackends => commands::info::cmd_list_backends(),
    }
}
