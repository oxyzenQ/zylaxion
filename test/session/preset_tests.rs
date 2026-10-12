// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// preset_tests.rs — the session/preset.rs resolution pins
// (night-improve-22, the no-config law). Replaces the TOML-era
// config_tests suite: the resolver's whole contract is now the
// priority chain (CLI flag > ZYLAXION_PRESET env > compiled default)
// plus the hard-error rule for unknown names and bad volumes.
//
// The suite pins the PURE cores (resolve_from / master_volume_from /
// context_preset_name_from) — no process-env mutation, so no race
// against concurrent env readers (the edition-2024 hazard; a first
// draft of this suite mutated ZYLAXION_* under ENV_LOCK and still
// flaked against readers of other variables). Exactly ONE integration
// test covers the env-reading shell, serialized behind ENV_LOCK.

use super::*;

// The ENV_LOCK moved here when pathguard died with the daemon
// (night-improve-23): the mutex exists only for this suite's single
// env-reading integration test.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn resolve_cli_flag_wins_over_everything() {
    let (_profile, name, _volume) =
        resolve_from(Some("ibm_model_m"), Some("topre"), None).expect("cli flag must win");
    assert_eq!(name, "ibm_model_m");
}

#[test]
fn resolve_env_selects_preset_without_cli() {
    let (_profile, name, _volume) =
        resolve_from(None, Some("topre"), None).expect("env selection must resolve");
    assert_eq!(name, "topre");
}

#[test]
fn resolve_defaults_without_cli_or_env() {
    let (_profile, name, _volume) = resolve_from(None, None, None).expect("default must resolve");
    assert_eq!(name, DEFAULT_PRESET);
    assert_eq!(name, "technical");
}

#[test]
fn resolve_empty_env_value_counts_as_unset() {
    let (_profile, name, _volume) =
        resolve_from(None, Some("   "), None).expect("blank env must fall through");
    assert_eq!(name, DEFAULT_PRESET);
}

#[test]
fn resolve_unknown_cli_name_is_hard_error_with_tip() {
    let err = resolve_from(Some("ibm_model_"), None, None).expect_err("near-miss must fail");
    assert!(err.contains("Preset 'ibm_model_' not found"), "{err}");
    assert!(err.contains("the --preset flag"), "{err}");
    assert!(err.contains("technical"), "{err}");
    assert!(err.contains("tip: did you mean 'ibm_model_m'?"), "{err}");
}

#[test]
fn resolve_unknown_env_name_is_hard_error_with_tip() {
    let err = resolve_from(None, Some("cherrym"), None).expect_err("bad env name must fail");
    assert!(
        err.contains("ZYLAXION_PRESET environment variable"),
        "{err}"
    );
    assert!(err.contains("tip: did you mean 'cherryMX'?"), "{err}");
}

#[test]
fn resolve_env_name_with_whitespace_trims() {
    let (_profile, name, _volume) =
        resolve_from(None, Some("  classic  "), None).expect("trimmed env must resolve");
    assert_eq!(name, "classic");
}

#[test]
fn resolve_is_case_sensitive_like_the_old_config() {
    let err = resolve_from(Some("CherryMX"), None, None).expect_err("case mismatch must fail");
    assert!(err.contains("tip: did you mean 'cherryMX'?"), "{err}");
}

#[test]
fn resolve_carries_the_volume_through() {
    let (_profile, _name, volume) =
        resolve_from(None, None, Some("1.5")).expect("resolution with volume must succeed");
    assert_eq!(volume, 1.5);
}

#[test]
fn resolve_bad_volume_fails_even_with_valid_preset() {
    let err = resolve_from(Some("technical"), None, Some("loud"))
        .expect_err("bad volume must fail the whole resolution");
    assert!(
        err.contains("invalid ZYLAXION_VOLUME value 'loud'"),
        "{err}"
    );
}

#[test]
fn master_volume_defaults_to_the_compiled_value() {
    assert_eq!(master_volume_from(None).unwrap(), 5.5);
    assert_eq!(
        master_volume_from(None).unwrap(),
        crate::engine::VoicePool::DEFAULT_MASTER_VOLUME
    );
}

#[test]
fn master_volume_empty_value_counts_as_unset() {
    assert_eq!(master_volume_from(Some("")).unwrap(), 5.5);
    assert_eq!(master_volume_from(Some("   ")).unwrap(), 5.5);
}

#[test]
fn master_volume_parses_env_value() {
    assert_eq!(master_volume_from(Some("1.5")).unwrap(), 1.5);
    assert_eq!(master_volume_from(Some(" 3.0 ")).unwrap(), 3.0);
}

#[test]
fn master_volume_invalid_value_is_hard_error() {
    let err = master_volume_from(Some("loud")).expect_err("non-numeric volume must fail");
    assert!(
        err.contains("invalid ZYLAXION_VOLUME value 'loud'"),
        "{err}"
    );
    assert!(err.contains("must be a number"), "{err}");
}

#[test]
fn master_volume_non_finite_value_is_hard_error() {
    assert!(master_volume_from(Some("inf")).is_err());
    assert!(master_volume_from(Some("nan")).is_err());
}

#[test]
fn context_preset_name_follows_env_then_default() {
    assert_eq!(context_preset_name_from(None), DEFAULT_PRESET);
    assert_eq!(context_preset_name_from(Some("topre")), "topre");
    assert_eq!(context_preset_name_from(Some("  classic ")), "classic");
}

#[test]
fn context_preset_name_falls_back_on_unknown_env() {
    // The context probe never fails — the diagnostic belongs to
    // resolve(), which a real session start always hits.
    assert_eq!(
        context_preset_name_from(Some("not_a_preset")),
        DEFAULT_PRESET
    );
    assert_eq!(context_preset_name_from(Some("")), DEFAULT_PRESET);
}

#[test]
fn env_shell_resolves_the_real_environment() {
    // The one integration pin over the env-reading shell: ZYLAXION_PRESET
    // set in the REAL process environment must flow into resolve().
    // Serialized behind ENV_LOCK and snapshotted/restored like every
    // env-mutating test in the crate (pathguard contract).
    let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let saved = std::env::var(PRESET_ENV_VAR).ok();
    unsafe { std::env::set_var(PRESET_ENV_VAR, "topre") };
    let (_profile, name, _volume) = resolve(None).expect("env shell must resolve");
    match saved {
        Some(v) => unsafe { std::env::set_var(PRESET_ENV_VAR, v) },
        None => unsafe { std::env::remove_var(PRESET_ENV_VAR) },
    }
    assert_eq!(name, "topre");
}

#[test]
fn no_config_file_is_ever_read() {
    // The no-config law, pinned: the resolver's contract is env + flag
    // only. The registry — not any filesystem surface — is the source,
    // so the answer is identical no matter what the CWD contains.
    assert!(registry_lookup(DEFAULT_PRESET).is_some());
    let (_profile, name, _volume) = resolve_from(None, None, None).expect("resolve is fs-free");
    assert_eq!(name, DEFAULT_PRESET);
}
