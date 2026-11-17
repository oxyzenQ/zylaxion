// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-strict-warnings-1: the harness's mirror of the root crate's
//! zero-warning gate (root build.rs carries the full law). The same
//! env var emits the same cfg; the harness's crate root carries the
//! matching cfg_attr, and the #[path]-included zylaxion module trees
//! compile under it too. The gate rides the env var — NOT RUSTFLAGS —
//! so the alias/release rustflags sources stay untouched.

fn main() {
    println!("cargo:rerun-if-env-changed=ZYLAXION_STRICT_WARNINGS");
    // Declared for rustc's check-cfg machinery — see the root build.rs
    // law comment (an emitted rustc-cfg is not auto-expected).
    println!("cargo::rustc-check-cfg=cfg(zylaxion_strict_warnings)");
    if std::env::var("ZYLAXION_STRICT_WARNINGS").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=zylaxion_strict_warnings");
    }
}
