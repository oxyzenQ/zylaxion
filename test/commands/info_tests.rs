// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// info_tests.rs — the info-render pins (night-improve-29): the
// list-presets table is ONE line per preset, marked on the active
// row, aligned on one description column, and carries no how-to
// footer (the help and the overview own the teaching; a list lists).

use super::*;

/// The table renders exactly one line per registered preset: the
/// name starts its row and the description rides the same line.
#[test]
fn presets_table_is_one_line_per_preset() {
    let registry = crate::profiles::registry_all();
    let table = presets_table_from(crate::profiles::DEFAULT_PRESET, None);
    let rows: Vec<&str> = table
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.contains("note:"))
        .collect();
    assert_eq!(rows.len(), registry.len(), "one row per preset: {table}");
    for (entry, row) in registry.iter().zip(rows.iter()) {
        assert!(
            row.trim_start().starts_with(entry.name),
            "row for {}: {row}",
            entry.name
        );
        assert!(row.contains(entry.description), "inline: {row}");
    }
}

/// The active row is marked exactly once, at the end of its line —
/// and the marker lands on the row the session would activate.
#[test]
fn presets_table_marks_the_active_row_once() {
    let active = crate::profiles::DEFAULT_PRESET;
    let table = presets_table_from(active, None);
    assert_eq!(table.matches("<- active").count(), 1, "one marker: {table}");
    let marked = table
        .lines()
        .find(|l| l.contains("<- active"))
        .expect("the marker is present");
    assert!(
        marked.trim_start().starts_with(active),
        "marker on the active row: {marked}"
    );
}

/// The how-to footer is gone — `Switch preset:` blocks, the redundant
/// `Active preset:` restatement, and the volume tip do not ride a
/// list command.
#[test]
fn presets_table_carries_no_footer() {
    let table = presets_table_from(crate::profiles::DEFAULT_PRESET, None);
    for phrase in ["Switch preset:", "Active preset:", "ZYLAXION_VOLUME"] {
        assert!(
            !table.contains(phrase),
            "footer phrase {phrase} is gone: {table}"
        );
    }
}

/// The alignment contract: every description starts at the SAME
/// absolute column (the padded name column) — the masterclass table
/// shape.
#[test]
fn presets_table_aligns_the_description_column() {
    let registry = crate::profiles::registry_all();
    let width = registry.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let desc_col = 2 + width + 2; // indent + name column + gap
    let table = presets_table_from(crate::profiles::DEFAULT_PRESET, None);
    for row in table
        .lines()
        .filter(|l| l.starts_with("  ") && !l.contains("note:"))
    {
        assert!(
            !row[desc_col..].starts_with(' '),
            "description must start at column {desc_col}: {row:?}"
        );
    }
}

/// The honesty note: an env selection that names no registered
/// preset is named, not silently swallowed.
#[test]
fn presets_table_names_an_unregistered_env_override() {
    let table = presets_table_from(crate::profiles::DEFAULT_PRESET, Some("no-such-preset"));
    assert!(
        table.contains("names no registered preset"),
        "the note is present: {table}"
    );
    assert!(
        table.contains("no-such-preset"),
        "the env name rides: {table}"
    );
}

/// A registered env selection stays silent — the marker is the whole
/// story.
#[test]
fn presets_table_stays_silent_on_a_registered_env_override() {
    let registry = crate::profiles::registry_all();
    let name = registry[0].name;
    let table = presets_table_from(name, Some(name));
    assert!(
        !table.contains("note:"),
        "no note for a registered override: {table}"
    );
}
