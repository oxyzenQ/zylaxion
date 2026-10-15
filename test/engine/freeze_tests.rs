// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The freeze pin (night-private-research-2xblade-4): the single test
//! that keeps the freeze from being silently undone. It asserts the
//! declaration is documented on all three surfaces — the README line
//! under the preset tables, the CHANGELOG freeze entry with its
//! verbatim honest sentence, and the engine module doc comment. The
//! certified commit hash is embedded as the contract it is: the
//! frozen engine's documentation must keep naming the commit the
//! declaration was made against.

const README: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"));
const CHANGELOG: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/CHANGELOG.md"));
const ENGINE_MOD: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/engine/mod.rs"));

/// The certified engine commit: the head of
/// night-private-research-2xblade-3.6X, read from the tree at freeze
/// time (never assumed).
const FROZEN_HASH: &str = "c028ae6f907055807125c9a20c156cb16a3a2038";

/// The honest sentence the CHANGELOG entry must carry word for word.
/// Checked against a whitespace-normalized copy of the file: the
/// changelog wraps its lines, the contract is the words, not the
/// column count.
const FREEZE_SENTENCE: &str = "The Zactrix Engine is frozen as of c028ae6f907055807125c9a20c156cb16a3a2038. It was built, sharpened, measured, and audited across seven passes; every proof and every audit finding is recorded in docs/ZACTRIX_LTS.md and docs/ZACTRIX_SECURITY.md. From this commit forward, changes require a why that passes the maintenance bar: a security fix, a compatibility fix, or a measured improvement proven by the same audit harness. The engine is whole.";

fn whitespace_normalized(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The module doc's lines carry the `//!` marker between words; strip
/// it before normalizing so the pin matches the WORDS, not the
/// comment syntax.
fn doc_words_normalized(text: &str) -> String {
    whitespace_normalized(&text.replace("//!", " "))
}

#[test]
#[cfg_attr(
    miri,
    ignore = "UB-poor full-text scan (safe string work only) that miri
        interprets for minutes - the native CI lane owns the docs assertions"
)]
fn readme_names_the_freeze_and_the_admission_paths() {
    let flat = whitespace_normalized(README);
    assert!(
        flat.contains("LTS-frozen"),
        "the README freeze line is gone — the freeze must stay documented under the preset tables"
    );
    assert!(
        flat.contains("docs/ZACTRIX_LTS.md"),
        "the README freeze line must point at the LTS certification record"
    );
    assert!(
        flat.contains("docs/ZACTRIX_SECURITY.md"),
        "the README freeze line must point at the security audit record"
    );
    assert!(
        flat.contains("a security fix, a compatibility fix, or a measurable improvement"),
        "the README freeze line must name the three admission paths"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "UB-poor full-text scan (safe string work only) that miri
        interprets for ~20 minutes alone - the native CI lane owns the
        docs assertions"
)]
fn changelog_carries_the_freeze_declaration() {
    let flat = whitespace_normalized(CHANGELOG);
    assert!(
        CHANGELOG.contains("night-private-research-2xblade-4"),
        "the CHANGELOG freeze entry for 2xblade-4 is gone"
    );
    assert!(
        CHANGELOG.contains(FROZEN_HASH),
        "the CHANGELOG freeze entry must name the certified commit hash"
    );
    assert!(
        CHANGELOG.contains("2026-10-14"),
        "the CHANGELOG freeze entry must carry the declaration date"
    );
    assert!(
        flat.contains(FREEZE_SENTENCE),
        "the CHANGELOG freeze entry must carry the honest sentence word for word"
    );
}

#[test]
fn engine_module_doc_carries_the_freeze_sentence() {
    let flat = doc_words_normalized(ENGINE_MOD);
    assert!(
        flat.contains(&format!("Frozen as of {FROZEN_HASH}")),
        "the engine module doc must name the frozen commit"
    );
    assert!(
        flat.contains(
            "a security fix, a compatibility fix, or a measured improvement proven by the audit harness"
        ),
        "the engine module doc must carry the maintenance bar"
    );
}
