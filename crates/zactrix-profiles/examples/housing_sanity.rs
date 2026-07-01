// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Quick sanity test: render N samples with and without the housing
//! layer (mix=0) and verify the housing layer contributes audible
//! energy. This is a one-off dev test, NOT a unit test — run with:
//!   cargo run --example housing_sanity -p zactrix-profiles

use zactrix_profiles::{
    AcousticModel, HousingParams, KeyEvent, KeyProfile, MechanicalClick, SynthState,
};

fn render_energy(profile: KeyProfile, n: usize) -> f32 {
    let model = MechanicalClick::with_profile(profile, 44_100);
    let event = KeyEvent {
        scancode: 30,
        pressed: true,
        stereo_position: 0.0,
    };
    let p = model.get_profile(&event);
    let mut state = SynthState::default();
    model.init_state(&p, &mut state, event.stereo_position);

    let mut sum_sq = 0.0_f32;
    for _ in 0..n {
        let [l, r] = model.render_sample(&mut state);
        sum_sq += l * l + r * r;
        if !state.active {
            break;
        }
    }
    sum_sq / n as f32
}

fn main() {
    let base = KeyProfile {
        housing: HousingParams {
            frequency: 250.0,
            resonance: 2.5,
            mix: 0.8, // high mix — thock-heavy
        },
        ..KeyProfile::default()
    };

    let energy_with_housing = render_energy(base, 5000);

    let no_housing = KeyProfile {
        housing: HousingParams {
            frequency: 250.0,
            resonance: 2.5,
            mix: 0.0, // effectively mute the housing layer
        },
        ..KeyProfile::default()
    };
    let energy_without_housing = render_energy(no_housing, 5000);

    println!("Energy WITH housing (mix=0.8):    {energy_with_housing:.6e}");
    println!("Energy WITHOUT housing (mix=0.0): {energy_without_housing:.6e}");
    println!(
        "Ratio: {:.2}x (should be > 1.0, meaning housing contributes energy)",
        energy_with_housing / energy_without_housing
    );

    if energy_with_housing > energy_without_housing * 1.05 {
        println!("PASS: housing layer contributes audible energy");
    } else {
        println!("FAIL: housing layer did not contribute enough energy");
        std::process::exit(1);
    }
}
