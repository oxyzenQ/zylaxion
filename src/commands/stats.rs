// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The `stats` verb (night-improve-23): the kernel-side typing
//! telemetry render. Split out of sound.rs per the 600-LOC no-mercy
//! cap — the stats surface is its own concern (a read-only map dump
//! with a human render), not a control-plane write.

use std::process;

#[cfg(feature = "ebpf")]
use aya::maps::{Array, MapData};

#[cfg(feature = "ebpf")]
use crate::ebpf::pin;

#[cfg(feature = "ebpf")]
/// Handle `zylaxion stats` — the kernel-side typing telemetry.
///
/// The tap accumulated per-keycode press counters in the stats map at
/// zero userspace cost; this reads them and renders the top keys with
/// their evdev names.
#[cfg(feature = "ebpf")]
pub fn cmd_stats() {
    let data = match MapData::from_pin(pin::PIN_MAP_STATS) {
        Ok(d) => d,
        Err(e) => {
            crate::output::error(format!(
                "the tap is not attached (cannot open {}): {e}\n  \
                 tip: sudo zylaxion attach",
                pin::PIN_MAP_STATS
            ));
            process::exit(1);
        }
    };
    let owned = aya::maps::Map::Array(data);
    let stats: Array<MapData, u64> = match Array::try_from(owned) {
        Ok(s) => s,
        Err(e) => {
            crate::output::error(format!("the pinned stats map is not an array: {e}"));
            process::exit(1);
        }
    };

    let total_slot = 256u32;
    let total = stats.get(&total_slot, 0).unwrap_or(0);
    let mut rows: Vec<(u32, u64)> = Vec::new();
    for code in 0..255u32 {
        let count = stats.get(&code, 0).unwrap_or(0);
        if count > 0 {
            rows.push((code, count));
        }
    }
    rows.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    println!("Keyboard stats (accumulated in the kernel since attach):\n");
    println!("  total presses: {total}");
    if rows.is_empty() {
        println!("  no presses recorded yet — type something!");
        return;
    }
    println!("\n  top keys:");
    for (code, count) in rows.iter().take(10) {
        println!("  {:>8}  keycode {code:>3} ({})", count, key_name(*code));
    }
}

/// A compact evdev key name for the stats render. The common keyboard
/// keys carry real names; everything else renders as key<N> — the raw
/// count is the truth, the name is a courtesy.
fn key_name(code: u32) -> String {
    let common: &[(u32, &str)] = &[
        (1, "ESC"),
        (2, "1"),
        (3, "2"),
        (4, "3"),
        (5, "4"),
        (6, "5"),
        (7, "6"),
        (8, "7"),
        (9, "8"),
        (10, "9"),
        (11, "0"),
        (14, "BKSP"),
        (15, "TAB"),
        (16, "Q"),
        (17, "W"),
        (18, "E"),
        (19, "R"),
        (20, "T"),
        (21, "Y"),
        (22, "U"),
        (23, "I"),
        (24, "O"),
        (25, "P"),
        (26, "["),
        (27, "]"),
        (28, "ENTER"),
        (29, "LCTRL"),
        (30, "A"),
        (31, "S"),
        (32, "D"),
        (33, "F"),
        (34, "G"),
        (35, "H"),
        (36, "J"),
        (37, "K"),
        (38, "L"),
        (39, ";"),
        (40, "'"),
        (41, "`"),
        (42, "LSHIFT"),
        (43, "\\"),
        (44, "Z"),
        (45, "X"),
        (46, "C"),
        (47, "V"),
        (48, "B"),
        (49, "N"),
        (50, "M"),
        (51, ","),
        (52, "."),
        (53, "/"),
        (54, "RSHIFT"),
        (55, "KP*"),
        (56, "LALT"),
        (57, "SPACE"),
        (58, "CAPS"),
        (59, "F1"),
        (60, "F2"),
        (61, "F3"),
        (62, "F4"),
        (63, "F5"),
        (64, "F6"),
        (65, "F7"),
        (66, "F8"),
        (67, "F9"),
        (68, "F10"),
        (69, "NUM"),
        (70, "SCROLL"),
        (87, "F11"),
        (88, "F12"),
        (96, "KP-ENTER"),
        (97, "RCTRL"),
        (98, "KP/"),
        (100, "RALT"),
        (102, "HOME"),
        (103, "UP"),
        (104, "PGUP"),
        (105, "LEFT"),
        (106, "RIGHT"),
        (107, "END"),
        (108, "DOWN"),
        (109, "PGDN"),
        (110, "INS"),
        (111, "DEL"),
        (119, "PAUSE"),
    ];
    common
        .iter()
        .find(|(c, _)| *c == code)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("key{code}"))
}

/// Dormant lane (stats): this binary was built without the eBPF
/// loader (--no-default-features) — the honest refusal.
#[cfg(not(feature = "ebpf"))]
pub fn cmd_stats() {
    crate::output::error(crate::ebpf::dormant::DORMANT_REFUSAL);
    process::exit(1);
}
