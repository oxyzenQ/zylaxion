// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-strict-warnings-1: the fuzz crate's mirror of the root crate's
//! zero-warning gate (root build.rs carries the full law). The same env
//! var emits the same cfg; every fuzz target crate root carries the
//! matching cfg_attr, so the path-included zylaxion module trees the
//! targets exercise are denied with them. The gate rides the env var —
//! NOT RUSTFLAGS — because cargo-fuzz builds its sanitizer flags
//! through RUSTFLAGS, and an env RUSTFLAGS would clobber the pro-*
//! alias CPU tiers if it ever rode a shared lane.

fn main() {
    println!("cargo:rerun-if-env-changed=ZYLAXION_STRICT_WARNINGS");
    // Declared for rustc's check-cfg machinery — see the root build.rs
    // law comment (an emitted rustc-cfg is not auto-expected).
    println!("cargo::rustc-check-cfg=cfg(zylaxion_strict_warnings)");
    if std::env::var("ZYLAXION_STRICT_WARNINGS").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=zylaxion_strict_warnings");
    }
}
