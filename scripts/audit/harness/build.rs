// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! night-arc-gems.2: the harness's build script. The zero-warning gate
//! itself is unconditional now — the crate root carries
//! `#![deny(warnings)]` directly (the env-gated mirror of
//! night-strict-warnings-1 is gone: a local audit build printed what
//! CI never saw, and that hole is closed in the source). What remains
//! here is the includer marker: the harness #[path]-includes the root
//! crate's module trees, and the profiles bridge exempts
//! `registry_names` (consumed by the session/commands trees, outside
//! this audit surface) under `zylaxion_audit_surface` — this build
//! emits that marker unconditionally, because this crate IS the audit
//! surface.

fn main() {
    // Declared for rustc's check-cfg machinery — an emitted rustc-cfg
    // is not auto-expected, and under the deny gate the
    // unexpected_cfgs warning would fail the build it polices.
    println!("cargo::rustc-check-cfg=cfg(zylaxion_audit_surface)");
    println!("cargo:rustc-cfg=zylaxion_audit_surface");
}
