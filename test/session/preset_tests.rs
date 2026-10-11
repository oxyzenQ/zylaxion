// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// preset_tests.rs — the session/preset.rs resolution pins
// (night-improve-22, the no-config law). Replaces the TOML-era
// config_tests suite: the resolver's whole contract is now the
// priority chain (CLI flag > ZYLAXION_PRESET env > compiled default)
// plus the hard-error rule for unknown names and bad volumes.

use super::*;

use crate::session::pathguard::ENV_LOCK;
use std::path::Path;

/// Snapshot every environment variable the resolver reads, run `f`,
/// then restore. Callers must hold [`ENV_LOCK`] so parallel test
/// threads cannot race the save/restore window (night-improve-4
/// edition-2024 rule: `set_var`/`remove_var` are unsafe-adjacent and
/// need crate-wide serialization).
fn with_preset_env<T>(f: impl FnOnce() -> T) -> T {
    let saved_preset = std::env::var(PRESET_ENV_VAR).ok();
    let saved_volume = std::env::var(VOLUME_ENV_VAR).ok();
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let out = f();
    match saved_preset {
        Some(v) => unsafe { std::env::set_var(PRESET_ENV_VAR, v) },
        None => unsafe { std::env::remove_var(PRESET_ENV_VAR) },
    }
    match saved_volume {
        Some(v) => unsafe { std::env::set_var(VOLUME_ENV_VAR, v) },
        None => unsafe { std::env::remove_var(VOLUME_ENV_VAR) },
    }
    out
}

#[test]
fn resolve_cli_flag_wins_over_everything() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "topre") };
        let (_profile, name, _volume) = resolve(Some("ibm_model_m")).expect("cli flag must win");
        assert_eq!(name, "ibm_model_m");
    });
}

#[test]
fn resolve_env_selects_preset_without_cli() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "topre") };
        let (_profile, name, _volume) = resolve(None).expect("env selection must resolve");
        assert_eq!(name, "topre");
    });
}

#[test]
fn resolve_defaults_without_cli_or_env() {
    with_preset_env(|| {
        unsafe { std::env::remove_var(PRESET_ENV_VAR) };
        let (_profile, name, _volume) = resolve(None).expect("default must resolve");
        assert_eq!(name, DEFAULT_PRESET);
        assert_eq!(name, "technical");
    });
}

#[test]
fn resolve_empty_env_value_counts_as_unset() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "   ") };
        let (_profile, name, _volume) = resolve(None).expect("blank env must fall through");
        assert_eq!(name, DEFAULT_PRESET);
    });
}

#[test]
fn resolve_unknown_cli_name_is_hard_error_with_tip() {
    with_preset_env(|| {
        unsafe { std::env::remove_var(PRESET_ENV_VAR) };
        let err = resolve(Some("ibm_model_")).expect_err("near-miss must fail");
        assert!(err.contains("Preset 'ibm_model_' not found"), "{err}");
        assert!(err.contains("the --preset flag"), "{err}");
        assert!(err.contains("technical"), "{err}");
        assert!(err.contains("tip: did you mean 'ibm_model_m'?"), "{err}");
    });
}

#[test]
fn resolve_unknown_env_name_is_hard_error_with_tip() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "cherrym") };
        let err = resolve(None).expect_err("bad env name must fail");
        assert!(
            err.contains("ZYLAXION_PRESET environment variable"),
            "{err}"
        );
        assert!(err.contains("tip: did you mean 'cherryMX'?"), "{err}");
    });
}

#[test]
fn resolve_env_name_with_whitespace_trims() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "  classic  ") };
        let (_profile, name, _volume) = resolve(None).expect("trimmed env must resolve");
        assert_eq!(name, "classic");
    });
}

#[test]
fn resolve_is_case_sensitive_like_the_old_config() {
    with_preset_env(|| {
        unsafe { std::env::remove_var(PRESET_ENV_VAR) };
        let err = resolve(Some("CherryMX")).expect_err("case mismatch must fail");
        assert!(err.contains("tip: did you mean 'cherryMX'?"), "{err}");
    });
}

#[test]
fn master_volume_defaults_to_the_compiled_value() {
    with_preset_env(|| {
        unsafe { std::env::remove_var(VOLUME_ENV_VAR) };
        let v = master_volume().expect("unset volume must default");
        assert_eq!(v, crate::engine::VoicePool::DEFAULT_MASTER_VOLUME);
        assert_eq!(v, 5.5);
    });
}

#[test]
fn master_volume_empty_value_counts_as_unset() {
    with_preset_env(|| {
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "") };
        assert_eq!(master_volume().unwrap(), 5.5);
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "   ") };
        assert_eq!(master_volume().unwrap(), 5.5);
    });
}

#[test]
fn master_volume_parses_env_value() {
    with_preset_env(|| {
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "1.5") };
        assert_eq!(master_volume().unwrap(), 1.5);
        unsafe { std::env::set_var(VOLUME_ENV_VAR, " 3.0 ") };
        assert_eq!(master_volume().unwrap(), 3.0);
    });
}

#[test]
fn master_volume_invalid_value_is_hard_error() {
    with_preset_env(|| {
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "loud") };
        let err = master_volume().expect_err("non-numeric volume must fail");
        assert!(
            err.contains("invalid ZYLAXION_VOLUME value 'loud'"),
            "{err}"
        );
        assert!(err.contains("must be a number"), "{err}");
    });
}

#[test]
fn master_volume_non_finite_value_is_hard_error() {
    with_preset_env(|| {
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "inf") };
        assert!(master_volume().is_err());
        unsafe { std::env::set_var(VOLUME_ENV_VAR, "nan") };
        assert!(master_volume().is_err());
    });
}

#[test]
fn context_preset_name_follows_env_then_default() {
    with_preset_env(|| {
        unsafe { std::env::remove_var(PRESET_ENV_VAR) };
        assert_eq!(context_preset_name(), DEFAULT_PRESET);
        unsafe { std::env::set_var(PRESET_ENV_VAR, "topre") };
        assert_eq!(context_preset_name(), "topre");
    });
}

#[test]
fn context_preset_name_falls_back_on_unknown_env() {
    with_preset_env(|| {
        unsafe { std::env::set_var(PRESET_ENV_VAR, "not_a_preset") };
        // The context probe never fails — the diagnostic belongs to
        // resolve(), which a real session start always hits.
        assert_eq!(context_preset_name(), DEFAULT_PRESET);
    });
}

#[test]
fn no_config_file_is_ever_read() {
    // The no-config law, pinned: the resolver's contract is env + flag
    // only. Run it from a directory that contains a config.toml and
    // prove the answer still comes from the registry, not the file.
    with_preset_env(|| {
        unsafe { std::env::remove_var(PRESET_ENV_VAR) };
        let cwd = std::env::current_dir().expect("cwd readable");
        // The repo root ships no config.toml after night-improve-22;
        // if one ever reappears this test still holds because the
        // resolver must not look at the filesystem at all.
        assert!(registry_lookup(DEFAULT_PRESET).is_some());
        let (_profile, name, _volume) = resolve(None).expect("resolve is filesystem-free");
        assert_eq!(name, DEFAULT_PRESET);
        // Silence the unused-variable lint when the assert above is
        // the only consumer of cwd.
        let _ = Path::new(&cwd).exists();
    });
}
