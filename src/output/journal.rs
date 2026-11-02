// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The journal lane (night-dragon-maintenance-xenric-1): the stderr
//! surface the `log` facade renders through env_logger, wired through
//! ONE door.
//!
//! Three verbs log through the facade — attach, the sound verbs, the
//! player — and each used to carry its own copy of the same builder
//! closure (the hand-rolled UTC stamp, the level/target column, no
//! time crate). Three copies of one format can drift silently; this
//! module is the one door they all open, the same one-door discipline
//! the color tiers above enforce for their lanes: the journal IS an
//! output lane, the stderr sibling of the color tiers this group owns.
//!
//! The caller supplies only the default filter — the per-verb
//! loudness stays a call-site decision, visible in the file that owns
//! the verb:
//!
//!   - `attach` runs at `info`: the chain's per-step verdicts (the
//!     fentry refusal, the kprobe fallbacks) ARE the user-facing
//!     feedback of the privileged one-shot; the summary line is the
//!     other half of the same story.
//!   - the sound verbs and the player run at `warn` (night-hunter-6):
//!     the player is a detached background process whose stderr dies
//!     at spawn, and the state transitions an info default prints are
//!     noise without a reader. `RUST_LOG=info` restores the full
//!     story in a foreground `zylaxion _play` run — the diagnostic
//!     lane the parent's failure messages already name.
//!
//! `RUST_LOG` still overrides any default (env_logger's `from_env`
//! contract, the documented debug surface).

/// Wire the journal lane. Called at most once per process, by the
/// verb handlers, before any `log::` call on the path — a second
/// `init` anywhere is a bug env_logger itself refuses loudly.
pub(crate) fn init(default_filter: &str) {
    use std::io::Write as _;
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or(default_filter))
        .format(|buf, record| {
            writeln!(
                buf,
                "[{} {:<5} {}] {}",
                crate::info::clock::iso_utc_millis(),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
}
