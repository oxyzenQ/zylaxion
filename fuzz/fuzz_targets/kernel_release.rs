// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Fuzz target: the kernel-release parser (night-dragon-maintenance-5).
//!
//! `/proc/sys/kernel/osrelease` is host-controlled text: a corrupted
//! or hostile environment can feed zylaxion any byte stream as the
//! kernel release, and the floor verdict must answer Some or None,
//! never a panic. The harness drives the pure pair end to end -
//! [`parse_kernel_release`] and [`is_supported`] - over arbitrary
//! bytes, including the hostile classes: empty strings, huge fields,
//! negative numbers, non-ASCII bytes, and the real-world shapes the
//! committed seeds carry (the jammy GA release, the WSL2 suffix, the
//! below-floor 5.4 line).
//!
//! The source is path-included, not copied (see fuzz/Cargo.toml for
//! the full rationale): the harness compiles the same bytes the
//! binary ships, so the guarantee the lane proves is the guarantee
//! the binary carries.

#![no_main]

// The include law pulls the whole file; the harness exercises the
// release-parse + floor pair, not the live probe surface next to it
// (the probe reads /proc and /sys - the fuzzer's job is the grammar).
// The allow rides the HARNESS module (never the shipped source).
#[allow(dead_code)]
#[path = "../../src/session/kernel.rs"]
mod kernel;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The parser's domain is text; hostile bytes fail the UTF-8 check
    // before the grammar ever sees them (the same gate /proc carries).
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    // The contract: Some or None, never a panic.
    let parsed = kernel::parse_kernel_release(text);
    let supported = kernel::is_supported(parsed);
    // The floor law, fuzzed: a parsed (major, minor) meets the 5.15
    // floor exactly when the verdict says so. Any drift between the
    // parser's arithmetic and the floor comparison is a bug.
    if let Some((major, minor)) = parsed {
        assert_eq!(
            supported,
            major > 5 || (major == 5 && minor >= 15),
            "floor verdict drifted from the parsed (major, minor): \
             {major}.{minor} reported supported={supported}"
        );
    } else {
        // An unparseable release is honest None - and None must never
        // read as supported (the floor's fail-closed direction).
        assert!(!supported, "None parsed but reported supported");
    }
});
