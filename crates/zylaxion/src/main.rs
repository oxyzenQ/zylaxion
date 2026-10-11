// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! **zylaxion** — real-time mechanical keyboard acoustic synthesizer for Linux.
//!
//! Transforms every keystroke into a spatially-accurate click sound
//! through your speakers, using the kernel's evdev interface and
//! low-latency audio via cpal / PipeWire.
//!
//! # Architecture (night-improve-14, the zelynic foundation shape)
//!
//! `src/main.rs` is the ONLY file at the src root — bootstrap + dispatch;
//! every concern lives in its own folder:
//!
//! - [`cli`]       — clap definitions, the version report, clap styles
//! - [`output`]    — the color/lane system (errors, warnings, brand tiers)
//! - [`info`]      — the binary's self-knowledge: build facts, Hinnant clock
//! - [`session`]   — the session contract: config, instance lock,
//!   pathguard, privilege, signals
//! - [`commands`]  — subcommand handlers (`start`, `daemon`, `stop`, `doctor`, …)
//! - [`daemon`]    — POSIX daemonization, PID file, IPC

use clap::Parser;

mod cli;
mod commands;
mod daemon;
mod info;
mod output;
mod session;

fn main() {
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

    // -V / --version (night-improve-13): intercepted when it is the FIRST
    // argument — the documented invocation shape (`zylaxion -V`) — so the
    // version report renders through the brand lane (purple TTY header via
    // new stdout lanes; clap path shares one body), instead of clap's
    // plain dump. Any other position falls through to clap, whose own
    // version rendering carries the same report body (the two surfaces
    // share one body via cli::version_body, they cannot drift on facts).
    if std::env::args()
        .nth(1)
        .is_some_and(|a| a == "-V" || a == "--version")
    {
        cli::print_version_report();
        std::process::exit(0);
    }

    let cli = cli::Cli::parse();

    // Early-exit surfaces (night-improve-15, the zelynic dispatch
    // order): the curated help first, then the network check. Both
    // are real parsed fields now — the former argv peeks (and the
    // dead clap fields behind them) are gone.
    if cli.help {
        commands::help::print_reference();
        std::process::exit(0);
    }
    if cli.check_update {
        commands::update::run_check_update();
        std::process::exit(0);
    }

    match cli.command {
        Some(cmd) => match cmd {
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
        },
        None => {
            if cli.live {
                commands::info::cmd_live_overview();
            } else {
                commands::info::cmd_overview();
            }
        }
    }
}
