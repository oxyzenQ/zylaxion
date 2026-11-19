// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! The tap object's build-time digest stamp (night-depth-hardening-Z1
//! D2), split out of build/ebpf.rs per the 600-LOC no-mercy cap (the
//! camera.rs precedent of the zejtron reference, night-frontier-Z5:
//! the concern gets its own module, a pure move, gates green in
//! between). BOTH staging lanes call this the moment the object lands
//! in OUT_DIR, so the digest always describes the exact bytes the host
//! crate embeds - one stamp per lane, one writer of the env.
//!
//! Why the stamp exists: until now the object's blake3 was a BUILD-time
//! fact only (the registry lane's manifest pin in build/prebuilt.rs,
//! the source lane's structural validation) - the binary itself never
//! proved that the object it carries is the object those lanes staged.
//! A post-build patch of the embedded copy (the one artifact that
//! executes at ring 0) rode every later run silently: the identity
//! layers verified, the release checksum described the pre-patch file.
//! The stamp gives the runtime the anchor to re-hash against: `sig`'s
//! D4 tap-object row recomputes the digest over the bytes the binary
//! actually carries and compares them to this env, so a patched tap is
//! refused BY NAME on the verification surface.

use std::path::Path;

use crate::prebuilt::STAGED_OBJECT_NAME;

/// BLAKE3 over the staged object bytes, stamped
/// `ZYLAXION_TAP_OBJECT_B3` so the runtime can re-hash what it
/// embedded and refuse a patched tap (the sig surface's D4 tap-object
/// row - the eBPF object is the one artifact that executes at ring 0,
/// so it belongs inside the self-verification circle, not only inside
/// the release checksum files). The same std-only BLAKE3 the prebuilt
/// lane's manifest verification rides - one implementation, build side
/// and registry side.
pub(super) fn stamp_tap_digest(out_dir: &Path) {
    let staged = out_dir.join(STAGED_OBJECT_NAME);
    let data = std::fs::read(&staged).unwrap_or_else(|e| {
        panic!(
            "failed to read the staged {} for the tap digest stamp: {e}",
            staged.display()
        )
    });
    println!(
        "cargo:rustc-env=ZYLAXION_TAP_OBJECT_B3={}",
        crate::blake3::blake3_hex(&data)
    );
}
