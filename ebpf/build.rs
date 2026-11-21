// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-strict-warnings-1: the eBPF tap's mirror of the root crate's
//! zero-warning gate (root build.rs carries the full law). The same
//! env var emits the same cfg; the tap's crate root carries the
//! matching cfg_attr, so the kernel-side source answers the same
//! one-warning-fails law as the userspace estate. The gate rides the
//! env var — NOT RUSTFLAGS — so the nightly -Z build-std cross-build
//! and its toolchain contract stay untouched. The script runs on the
//! HOST even though the crate cross-compiles to bpfel-unknown-none
//! (cargo always runs build scripts on the host triple).

fn main() {
    println!("cargo:rerun-if-env-changed=ZYLAXION_STRICT_WARNINGS");
    // Declared for rustc's check-cfg machinery — see the root build.rs
    // law comment (an emitted rustc-cfg is not auto-expected).
    println!("cargo::rustc-check-cfg=cfg(zylaxion_strict_warnings)");
    if std::env::var("ZYLAXION_STRICT_WARNINGS").as_deref() == Ok("1") {
        println!("cargo:rustc-cfg=zylaxion_strict_warnings");
    }
}
