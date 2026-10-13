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

// ── night-improve-27: the volume gate and the env-lane gain cap ──────

/// The gate's honest domain: 0..=200 passes without the override,
/// every boundary value included.
#[test]
fn volume_gate_accepts_the_safety_domain() {
    for percent in [0u32, 1, 100, 199, 200] {
        assert_eq!(
            volume_gate(percent, false).expect("within the safety domain"),
            percent,
            "{percent}% must pass without the override"
        );
    }
}

/// Above the safety cap, the gate refuses and names the cap, the
/// reason, and the escape.
#[test]
fn volume_gate_refuses_above_the_cap_naming_the_escape() {
    let err = volume_gate(201, false).expect_err("201 must refuse");
    assert!(err.contains("exceeds the 200% safety cap"), "{err}");
    assert!(err.contains("can damage speakers"), "{err}");
    assert!(err.contains("Use --force-this to override"), "{err}");
    let err = volume_gate(1000, false).expect_err("1000 must refuse");
    assert!(
        err.contains("volume 1000% exceeds the 200% safety cap"),
        "{err}"
    );
}

/// The override lifts the ceiling to the force cap: 201..=10000 pass
/// with --force-this, and the force ceiling itself passes.
#[test]
fn volume_gate_force_lifts_to_the_force_cap() {
    for percent in [201u32, 500, 1000, 9999, 10000] {
        assert_eq!(
            volume_gate(percent, true).expect("within the force domain"),
            percent,
            "{percent}% must pass with the override"
        );
    }
}

/// Past the force cap nothing passes — with or without the override —
/// and the refusal names the force cap as the ceiling.
#[test]
fn volume_gate_refuses_past_the_force_cap() {
    let err = volume_gate(10001, true).expect_err("10001 must refuse");
    assert!(
        err.contains("exceeds the 10000% force cap"),
        "the refusal names the force cap: {err}"
    );
    let err = volume_gate(u32::MAX, true).expect_err("u32::MAX must refuse");
    assert!(err.contains("10000% force cap"), "{err}");
}

/// The env lane's gain cap: 10.0 is the loudest ZYLAXION_VOLUME; the
/// refusal rides the same wording pattern.
#[test]
fn master_volume_env_gain_is_capped_at_ten() {
    assert_eq!(
        master_volume_from(Some("10.0")).expect("the cap is inclusive"),
        10.0
    );
    assert_eq!(
        master_volume_from(Some("5.5")).expect("the compiled default"),
        5.5
    );
    let err = master_volume_from(Some("10.1")).expect_err("above the gain cap must refuse");
    assert!(err.contains("exceeds the 10 gain safety cap"), "{err}");
    assert!(err.contains("can damage speakers"), "{err}");
    let err = master_volume_from(Some("42")).expect_err("way above must refuse too");
    assert!(
        err.contains("gain 42 exceeds the 10 gain safety cap"),
        "{err}"
    );
}

/// The env lane maps into the percent domain honestly: the 10.0 gain
/// cap lands at 182 percent — inside the percent lane's safety cap,
/// two scales, both capped.
#[test]
fn the_env_gain_cap_maps_inside_the_percent_cap() {
    let percent = volume_percent_from_gain(10.0).expect("the gain cap maps honestly");
    assert_eq!(percent, 182, "10.0 gain = 182 percent of the 5.5 default");
    let err = volume_percent_from_gain(-1.0).expect_err("below silence refuses");
    assert!(err.contains("below silence"), "{err}");
}
