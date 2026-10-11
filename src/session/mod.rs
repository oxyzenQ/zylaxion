// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The session contract (night-improve-14, the zelynic concern-folder
//! discipline): everything that binds zylaxion to the invoking user's
//! session. zylaxion is a user-session program — every byte of state
//! it writes belongs inside that session, and each module here guards
//! one edge of that promise:
//!
//! - [`config`] — which config.toml governs (the session-scoped
//!   search path: ~/.config, /etc, /usr/local/share, ./).
//! - [`instance_lock`] — at most one zylaxion per session (the flock).
//! - [`pathguard`] — WHERE runtime state may live (the dangerous
//!   XDG_RUNTIME_DIR verdicts).
//! - [`privilege`] — WHO may run the privileged surfaces (the
//!   night-improve-16 root-usage audit).
//! - [`signals`] — how the session ends (graceful shutdown handlers).

pub mod config;
pub mod instance_lock;
pub mod pathguard;
pub mod privilege;
pub mod signals;
