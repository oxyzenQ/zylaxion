// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// help_tests.rs — the commands/help.rs test pins, relocated from the inline module (night-improve-18,
// the zelynic Pattern C test tree: every *.rs test file lives
// under the crate's test/ directory, wired via #[path]).
// The module position is unchanged, so super::* still resolves.

use super::*;

/// The integration-pin law (the zelynic help contract): every
/// subcommand the CLI exposes must appear in the reference, and
/// the group headings must hold — a new command or a reshuffle
/// without updating this module fails the suite.
#[test]
fn reference_carries_every_subcommand_and_group() {
    let text = render_reference(false);
    for command in [
        "attach",
        "on",
        "preset",
        "volume",
        "off",
        "detach",
        "status",
        "doctor",
        "list-presets",
        "list-backends",
    ] {
        assert!(
            text.contains(&format!("zylaxion {command}")),
            "reference must teach the command: {command}"
        );
    }
    for heading in [
        "Run (the eBPF control plane, night-improve-23):",
        "Diagnostics:",
        "Discover:",
        "Global flags:",
        "Pro mode:",
        "Advanced flags:",
    ] {
        assert!(
            text.contains(heading),
            "reference must carry the group heading: {heading}"
        );
    }
}

/// The tier law under color: headings purple, grammar grey,
/// examples green; under no color: zero escape bytes anywhere
/// (the pipe/`NO_COLOR` plain contract).
#[test]
fn tiers_carry_and_plain_mode_is_byte_clean() {
    let plain = render_reference(false);
    assert!(
        !plain.contains('\x1b'),
        "plain mode must carry zero escape bytes"
    );
    let painted = render_reference(true);
    for (needle, why) in [
        (BRAND_OPEN, "headings must ride purple"),
        (GREY_OPEN, "synopses must ride grey"),
        (OK_OPEN, "examples must ride green"),
    ] {
        assert!(painted.contains(needle), "{why}");
    }
    // Every open span is closed: the reset count matches the sum
    // of the three tier opens.
    let opens = painted.matches(BRAND_OPEN).count()
        + painted.matches(GREY_OPEN).count()
        + painted.matches(OK_OPEN).count();
    assert_eq!(
        painted.matches(RESET).count(),
        opens,
        "every span must close"
    );
}

/// The no-subcommand contract and the license footer close the
/// reference — the same facts the old clap `after_help` carried,
/// now on the one curated surface.
#[test]
fn reference_closes_with_contract_and_license() {
    let text = render_reference(false);
    assert!(
        text.contains("Without a subcommand: a quick status overview."),
        "the no-subcommand contract must be taught"
    );
    assert!(
        text.contains("License: GPL-3.0-only | https://github.com/oxyzenQ/zylaxion"),
        "the license footer must close the reference"
    );
}

/// night-improve-24: the merged status surface — the `stats` verb
/// died inside `status --verbose`, and `--live` died totally. The
/// reference must teach the deep-read spelling and must not teach
/// either dead one (a stale teaching would send users into an
/// unknown-argument error).
#[test]
fn reference_teaches_the_merged_status_and_no_dead_verbs() {
    let text = render_reference(false);
    assert!(
        text.contains("zylaxion status [--verbose]"),
        "the status synopsis must teach the verbose deep read"
    );
    assert!(
        !text.contains("zylaxion stats"),
        "the stats verb is merged into status --verbose — the reference must not teach it"
    );
    assert!(
        !text.contains("zylaxion --live"),
        "the --live flag is deleted — the reference must not teach it"
    );
}

/// night-improve-25: the reference must never teach the tracepoint
/// fiction again. The tap is a passive kernel-side probe on the
/// exported input_event() function (fentry when BTF is present,
/// kprobe otherwise) — the `input_event tracepoint` string is the
/// drift marker for a hook that never existed in mainline Linux, and
/// seeing it here means the help surface regressed to the removed
/// wrong path. This is the gate that keeps the drift from returning.
#[test]
fn reference_never_teaches_the_tracepoint_fiction() {
    let text = render_reference(false);
    assert!(
        !text.contains("input_event tracepoint"),
        "the reference must never teach the input_event tracepoint — \
         it does not exist in mainline Linux (night-improve-25)"
    );
    // Belt to the suspenders: no tracepoint teaching at all, under
    // either color mode (a painted string the plain pin cannot see).
    assert!(
        !render_reference(true).contains("input_event tracepoint"),
        "the colored reference must never teach it either"
    );
}

/// night-improve-25 (the zelynic NIGHT-improve-45 lineage): the Pro
/// mode drift pin — every hidden and advanced spelling the estate
/// owns must be documented COMPLETELY on the one reference surface,
/// or the section has silently thinned back to hidden. The estate:
/// the three short aliases, the four environment spellings of the
/// no-config law, and the one hidden verb — pinned to the exact
/// `_play` spelling clap accepts (task 5 fixed the drift; the bare
/// derived `play` must never be taught back).
#[test]
fn pro_mode_documents_every_hidden_and_advanced_spelling() {
    let text = render_reference(false);
    assert!(
        text.contains("Pro mode:"),
        "the Pro mode section heading must be present"
    );
    for spelling in [
        "-s, --switch <NAME>",
        "-p, --percent <0-200>",
        "-v, --verbose",
        "--force-this",
        "ZYLAXION_PRESET=<name>",
        "ZYLAXION_VOLUME=<gain>",
        "NO_COLOR=1",
        "RUST_LOG=<filter>",
        "zylaxion _play",
    ] {
        assert!(
            text.contains(spelling),
            "Pro mode must document the spelling '{spelling}'"
        );
    }
    // The discovery path a user copies from (NIGHT-improve-52's
    // lesson): every short alias carries a runnable example.
    for example in [
        "zylaxion preset -s topre",
        "zylaxion volume -p 60",
        "zylaxion status -v",
        "zylaxion volume -p 250 --force-this",
    ] {
        assert!(
            text.contains(example),
            "Pro mode must carry the example '{example}'"
        );
    }
    // The drift guard: the hidden verb is `_play` (the spelling the
    // spawn argument and the startup-error strings use); the bare
    // derived spelling must not be taught as a verb.
    assert!(
        !text.contains("zylaxion play"),
        "the hidden verb is 'zylaxion _play' — the bare 'zylaxion play' \
         spelling is an unknown subcommand and must never be taught"
    );
}

/// night-hunter-2: the banner subtitle IS the Cargo.toml description —
/// wrapped at render time, so the flattened reference must carry the
/// registry-facing description verbatim. A hand-edited second copy of
/// the prose fails this pin (the zelynic no-drift law).
#[test]
fn the_banner_subtitle_is_the_crate_description_verbatim() {
    let text = render_reference(false);
    let flattened = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let description = env!("CARGO_PKG_DESCRIPTION");
    assert!(
        flattened.contains(description),
        "the help banner must carry the Cargo.toml description: {description}"
    );
}

// ── night-improve-27: the daemon-era wording sweep ───────────────────

/// The dead-service sweep, pinned as a test (a filesystem walk, so it
/// rides cfg(not(miri)) like every other FS-touching leg — Miri's
/// isolation has no opendir, and its mandate is UB, not wording): the string
/// "systemctl --user restart zylaxion" appears nowhere in the living
/// tree — src/, ebpf/, and docs/. docs/archive/ is skipped BY LAW, not
/// by accident: it is the documented frozen-record class (the
/// docs/README map says so), and rewriting history to please a grep
/// would falsify the record. CHANGELOG.md sits at the repo root, out
/// of this test's scope, and its own mentions are the historical
/// record of the sweep that killed the wording.
#[cfg(not(miri))]
#[test]
fn the_dead_service_command_appears_nowhere_in_the_living_tree() {
    const FORBIDDEN: &str = "systemctl --user restart zylaxion";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut stack = vec![root.join("src"), root.join("ebpf"), root.join("docs")];
    let mut scanned = 0usize;
    while let Some(dir) = stack.pop() {
        let entries = std::fs::read_dir(&dir).expect("every scanned dir is readable");
        for entry in entries {
            let path = entry.expect("entry is readable").path();
            if path.is_dir() {
                // The frozen-record class: never rewritten, so never scanned.
                if path.file_name().is_some_and(|n| n == "archive") {
                    continue;
                }
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs" || e == "md") {
                let body = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("{} must be readable: {e}", path.display()));
                scanned += 1;
                assert!(
                    !body.contains(FORBIDDEN),
                    "the dead service command survives in {}",
                    path.display()
                );
            }
        }
    }
    assert!(
        scanned > 50,
        "the sweep must actually walk the tree, got {scanned}"
    );
}

/// The reframed idle diagnostic teaches the two actions and never
/// claims a stuck thread: the 300s line carries the off escape and
/// the honest liveness fork. (A file read — cfg(not(miri)) with the
/// sweep above.)
#[cfg(not(miri))]
#[test]
fn the_idle_heartbeat_teaches_type_or_off() {
    let core = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core/mod.rs"),
    )
    .expect("core source readable");
    assert!(
        core.contains("no key events in {}s — idle; type to hear the sound"),
        "the 300s line must teach type-or-off"
    );
    assert!(
        !core.contains("input thread may be stuck"),
        "the stuck-thread lie must stay dead"
    );
}
