// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The typing-telemetry render (night-improve-23 birthed it as the
//! standalone `stats` verb; night-improve-24 merged that verb into
//! `status --verbose` — one master status report, the deep read a
//! flag away). Split out of sound.rs per the 600-LOC no-mercy cap —
//! the telemetry render is its own concern (a read-only stats-map
//! dump with a human render), not a control-plane write.

use aya::maps::{Array, MapData};

use crate::ebpf::pin;

/// Append the kernel-side typing telemetry section to the status
/// report (`zylaxion status --verbose`, night-improve-24).
///
/// The tap accumulated per-keycode press counters in the stats map
/// at zero userspace cost; this reads them and renders the top keys
/// with their evdev names. A section, not a verdict: when the stats
/// map cannot be opened, the rest of the report is already on
/// screen, so this warns and lets the report finish instead of
/// hard-exiting — the standalone `stats` verb used to die here
/// because the map was its whole purpose; as a section it is a gap
/// to name, not a failure to own. night-hunter-8: the render rides
/// the broken-pipe-safe stdout lane (`status | head` must not kill
/// the process with SIGPIPE-by-panic), and the section header rides
/// the brand tier like the overview's headers.
pub(crate) fn print_telemetry() {
    let data = match MapData::from_pin(pin::PIN_MAP_STATS) {
        Ok(d) => d,
        Err(e) => {
            crate::output::warning(format!(
                "the telemetry map is not readable (cannot open {}): {e}",
                pin::PIN_MAP_STATS
            ));
            return;
        }
    };
    let owned = aya::maps::Map::Array(data);
    let stats: Array<MapData, u64> = match Array::try_from(owned) {
        Ok(s) => s,
        Err(e) => {
            crate::output::warning(format!("the pinned stats map is not an array: {e}"));
            return;
        }
    };

    let total_slot = 256u32;
    let total = stats.get(&total_slot, 0).unwrap_or(0);
    let mut rows: Vec<(u32, u64)> = Vec::new();
    // The per-key slots are 0..=255 (the total rides 256) — the scan
    // must cover the full keycode domain the loader maps and the tap
    // counts (night-hunter-1: the old 0..255 skipped keycode 255, whose
    // presses landed in the total but could never reach the top keys).
    for code in 0..256u32 {
        let count = stats.get(&code, 0).unwrap_or(0);
        if count > 0 {
            rows.push((code, count));
        }
    }
    rows.sort_unstable_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    crate::output::print_stdout_line("");
    crate::output::print_stdout_line(&crate::output::section(
        "telemetry (accumulated in the kernel since attach):",
    ));
    crate::output::print_stdout_line(&format!("  total presses: {total}"));
    if rows.is_empty() {
        crate::output::print_stdout_line("  no presses recorded yet — type something!");
        return;
    }
    crate::output::print_stdout_line("");
    crate::output::print_stdout_line("  top keys:");
    for (code, count) in rows.iter().take(10) {
        crate::output::print_stdout_line(&format!(
            "    {:>8}  keycode {code:>3} ({})",
            count,
            key_name(*code)
        ));
    }
}

/// A compact evdev key name for the telemetry render. The common
/// keyboard keys carry real names; everything else renders as
/// `key<N>` — the raw count is the truth, the name is a courtesy.
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
