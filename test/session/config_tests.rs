// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// config_tests.rs — the session/config.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

const TWO_PRESET_TOML: &str = r#"
[preset]
tuning = "technical"

[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[preset.classic]
[preset.classic.click]
frequency = 3200.0
resonance = 2.5
duration_ms = 2.5
amplitude = 0.7
[preset.classic.spring]
frequency = 1200.0
resonance = 4.0
mix = 0.75
[preset.classic.decay]
coefficient = 0.9992
voice_off_threshold = 0.00001
"#;

#[test]
fn parse_extracts_tuning_and_presets() {
    let parsed = parse_config(TWO_PRESET_TOML).expect("parse should succeed");
    assert_eq!(parsed.tuning.as_deref(), Some("technical"));
    assert_eq!(parsed.presets.len(), 2);
    assert!(parsed.presets.contains_key("technical"));
    assert!(parsed.presets.contains_key("classic"));
}

#[test]
fn parse_handles_missing_tuning() {
    let toml = r#"
[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001
"#;
    let parsed = parse_config(toml).expect("parse should succeed");
    assert!(parsed.tuning.is_none());
    assert_eq!(parsed.presets.len(), 1);
}

#[test]
fn determine_active_preset_cli_overrides_tuning() {
    let parsed = parse_config(TWO_PRESET_TOML).unwrap();
    let active = determine_active_preset(Some("classic"), &parsed);
    assert_eq!(active, "classic");
}

#[test]
fn determine_active_preset_uses_tuning_when_no_cli() {
    let parsed = parse_config(TWO_PRESET_TOML).unwrap();
    let active = determine_active_preset(None, &parsed);
    assert_eq!(active, "technical");
}

#[test]
fn determine_active_preset_defaults_when_no_tuning() {
    let toml = r#"
[preset.classic]
[preset.classic.click]
frequency = 3200.0
resonance = 2.5
duration_ms = 2.5
amplitude = 0.7
[preset.classic.spring]
frequency = 1200.0
resonance = 4.0
mix = 0.75
[preset.classic.decay]
coefficient = 0.9992
voice_off_threshold = 0.00001
"#;
    let parsed = parse_config(toml).unwrap();
    let active = determine_active_preset(None, &parsed);
    assert_eq!(active, DEFAULT_PRESET);
}

#[test]
fn resolve_config_with_existing_preset_succeeds() {
    if !Path::new("config.toml").exists() {
        return; // skip if not running from repo root
    }
    let (profiles, _path, active, _master) =
        resolve_config(Some("classic")).expect("should resolve classic");
    assert_eq!(active, "classic");
    assert_eq!(profiles.default.click.frequency, 3200.0);
}

#[test]
fn resolve_config_with_nonexistent_preset_errors() {
    if !Path::new("config.toml").exists() {
        return; // skip if not running from repo root
    }
    let result = resolve_config(Some("nonexistent"));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("Preset 'nonexistent' not found"));
    assert!(err.contains("Available:"));
    // "nonexistent" is beyond edit distance 2 of every real preset
    // name — the did-you-mean tip must stay silent for distant
    // values (the suggestion contract, night-improve-19).
    assert!(!err.contains("did you mean"));
}

#[test]
fn preset_not_found_error_carries_the_did_you_mean_tip() {
    use std::collections::HashMap;
    let mut presets = HashMap::new();
    presets.insert("thocky".to_string(), PresetEntry::default());
    presets.insert("clacky".to_string(), PresetEntry::default());
    let err = preset_not_found_error("thock", &presets);
    assert!(err.contains("Preset 'thock' not found"), "head: {err}");
    assert!(err.contains("Available: clacky, thocky"), "list: {err}");
    assert!(err.contains("tip: did you mean 'thocky'?"), "tip: {err}");
    // Exact-list names are not re-suggested for a distant typo.
    let err2 = preset_not_found_error("fortnights", &presets);
    assert!(!err2.contains("did you mean"), "distant: {err2}");
}

#[test]
fn resolve_config_uses_tuning_when_no_cli() {
    if !Path::new("config.toml").exists() {
        return;
    }
    let (_profiles, _path, active, _master) =
        resolve_config(None).expect("should resolve via tuning");
    assert_eq!(active, "technical");
}

#[test]
fn reload_preset_ignores_cli_preset_and_uses_tuning() {
    // The --preset CLI flag is for INITIAL load only. On reload,
    // preset.tuning from the file is the single source of truth.
    // This test verifies that passing Some("cherryMX") to
    // reload_preset does NOT override the file's tuning="technical".
    let path = Path::new("config.toml");
    if !path.exists() {
        return; // skip if not running from repo root
    }
    let (_profiles, active, _master) = reload_preset(path, Some("cherryMX"))
        .expect("should reload from tuning, ignoring cli_preset");
    // The file's tuning is "technical", so reload must return
    // "technical" — NOT "cherryMX" from the cli_preset arg.
    assert_eq!(active, "technical");
}

#[test]
fn reload_preset_uses_tuning_when_no_cli() {
    let path = Path::new("config.toml");
    if !path.exists() {
        return;
    }
    let (_profiles, active, _master) = reload_preset(path, None).expect("should reload via tuning");
    assert_eq!(active, "technical");
}

#[test]
fn reload_preset_follows_tuning_change_in_file() {
    // Simulate the user editing config.toml: write a temp file with
    // tuning = "classic", reload, and verify the active preset is
    // "classic" — even when cli_preset is Some("technical").
    let toml = r#"
[preset]
tuning = "classic"

[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001

[preset.classic]
[preset.classic.click]
frequency = 3200.0
resonance = 2.5
duration_ms = 2.5
amplitude = 0.7
[preset.classic.spring]
frequency = 1200.0
resonance = 4.0
mix = 0.75
[preset.classic.decay]
coefficient = 0.9992
voice_off_threshold = 0.00001
"#;
    let path = std::env::temp_dir().join(format!(
        "zylaxion-reload-test-{}-{}.toml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, toml).expect("write tmp");

    // Pass Some("technical") as cli_preset — reload MUST ignore it
    // and use the file's tuning = "classic".
    let (profiles, active, _master) =
        reload_preset(&path, Some("technical")).expect("should reload classic");
    assert_eq!(active, "classic");
    assert_eq!(profiles.default.click.frequency, 3200.0);

    let _ = std::fs::remove_file(&path);
}

#[test]
fn validate_config_str_accepts_good_file() {
    assert!(validate_config_str(TWO_PRESET_TOML).is_ok());
}

#[test]
fn validate_config_str_rejects_bad_tuning_reference() {
    let toml = r#"
[preset]
tuning = "nonexistent"

[preset.technical]
[preset.technical.click]
frequency = 4500.0
resonance = 2.0
duration_ms = 1.5
amplitude = 0.8
[preset.technical.spring]
frequency = 1800.0
resonance = 3.5
mix = 0.6
[preset.technical.decay]
coefficient = 0.9994
voice_off_threshold = 0.00001
"#;
    let result = validate_config_str(toml);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.contains("preset.tuning = 'nonexistent'"));
    assert!(err.contains("Available:"));
}

#[test]
fn validate_config_str_rejects_empty_file() {
    assert!(validate_config_str("").is_err());
}

#[test]
fn validate_config_str_rejects_no_presets() {
    let toml = r#"
[preset]
tuning = "technical"
"#;
    let result = validate_config_str(toml);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("no [preset.*] tables"));
}

#[test]
fn list_presets_returns_all_names() {
    let path = Path::new("config.toml");
    if !path.exists() {
        return;
    }
    let (path, active, names) = list_presets().expect("should list presets");
    assert!(path.is_absolute());
    assert!(!names.is_empty());
    assert!(names.contains(&active.to_string()));
}

#[test]
fn build_profile_clamps_invalid_values() {
    use crate::profiles::{ClickParams, DecayParams, SpringParams};
    let entry = PresetEntry {
        click: Some(ClickParams {
            frequency: 100_000.0, // above 8000
            resonance: 2.0,
            duration_ms: 1.5,
            amplitude: 0.8,
        }),
        spring: Some(SpringParams {
            frequency: 1800.0,
            resonance: 3.5,
            mix: 0.6,
        }),
        decay: Some(DecayParams {
            coefficient: 9999.0, // would cause infinite loop
            voice_off_threshold: 0.00001,
            coefficient_fast: 0.0,
            fast_samples_ms: 0.0,
        }),
        ambient: None,
        housing: None,
        keys: vec![],
    };
    let profiles = build_profile_from_entry(&entry);
    assert_eq!(profiles.default.click.frequency, 8000.0);
    assert!(profiles.default.decay.coefficient < 1.0);
}

#[test]
fn build_profile_merges_per_key_overrides() {
    use crate::profiles::{ClickParams, DecayParams, KeyOverride, SpringParams};
    let entry = PresetEntry {
        click: Some(ClickParams {
            frequency: 4500.0,
            resonance: 2.0,
            duration_ms: 1.5,
            amplitude: 0.8,
        }),
        spring: Some(SpringParams {
            frequency: 1800.0,
            resonance: 3.5,
            mix: 0.6,
        }),
        decay: Some(DecayParams {
            coefficient: 0.9994,
            voice_off_threshold: 0.00001,
            coefficient_fast: 0.0,
            fast_samples_ms: 0.0,
        }),
        ambient: None,
        housing: None,
        keys: vec![KeyOverride {
            scancode: 28,
            click: Some(crate::profiles::OverrideClick {
                frequency: Some(3000.0),
                ..Default::default()
            }),
            ..Default::default()
        }],
    };
    let profiles = build_profile_from_entry(&entry);
    assert_eq!(profiles.overrides.len(), 1);
    let enter = profiles.for_scancode(28);
    assert_eq!(enter.click.frequency, 3000.0);
    assert_eq!(enter.click.resonance, 2.0); // inherited
}
