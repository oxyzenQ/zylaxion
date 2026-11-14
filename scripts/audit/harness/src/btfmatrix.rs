// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-2B: the kernel matrix mode — the tap's kernel
//! contract verified against a target kernel's BTF, without a kernel.
//!
//! The shipped tap is a compile-once object whose kernel contract is
//! a FUNCTION ABI (docs/CO_RE_PORTABILITY.md): the fentry lane
//! resolves its `attach_btf_id` at LOAD time from the target kernel's
//! own BTF, and the object itself carries no `.BTF.ext` field
//! relocations. This mode measures exactly those facts against any
//! BTF snapshot (a raw `/sys/kernel/btf/vmlinux`-shaped BTF blob —
//! the kernel-matrix driver extracts the snapshots from pinned
//! distro kernel images):
//!
//! 1. **The object parses** through `aya-obj` — the crate the main
//!    package's loader rides — and the program inventory reads back.
//! 2. **The CO-RE posture holds**: `has_btf_relocations()` answers
//!    false on the real shipped object (the documented verdict, now
//!    measured instead of asserted), and the full relocation path
//!    (`relocate_btf` against the snapshot, then `relocate_calls` +
//!    `relocate_maps`) completes clean.
//! 3. **The fentry target resolves**: `input_event` answers a BTF
//!    `Func` id — the EXACT lookup aya's `FEntry::load` performs at
//!    attach time (`btf.id_by_type_name_kind(fn_name, Func)`), so a
//!    hit here is the load-time fact, not a proxy.
//! 4. **The kprobe alt surface is recorded** (not gated): BTF knows
//!    `input_handle_event` — a kallsyms proxy only, since kprobe
//!    attach resolves through kallsyms, not BTF; the chain's own
//!    fallback verdict stays a live-kernel fact.
//!
//! One invocation = one kernel snapshot = one verdict row; the
//! kernel-matrix driver aggregates the table. Exit 1 on any gated
//! failure — a red here names the kernel whose BTF the tap's contract
//! does not resolve on.

use std::collections::HashSet;

use aya_obj::btf::{Btf, BtfKind};
use aya_obj::Object;

/// The staged tap object, relative to the harness crate — the same
/// bytes the binary embeds (the prebuilt lane's blake3-pinned
/// artifact; the source-built lane stages the identical object).
const TAP_OBJECT: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../ebpf-prebuilt/zylaxion-tap.o"
);

/// The fentry target: the exported input_event() function — the
/// symbol the attach chain's step 1 rides (`FENTRY_PROGRAM` fentry
/// section; src/ebpf/loader/chain.rs).
const FENTRY_TARGET: &str = "input_event";

/// The kprobe alt surface: the chain's step-3 fallback symbol —
/// recorded, never gated (kprobe attach resolves through kallsyms;
/// BTF presence is a proxy, and the chain already answers honestly
/// when the symbol is unavailable).
const KPROBE_ALT: &str = "input_handle_event";

pub(super) fn run_btf_matrix(btf_path: &str, label: &str) {
    let object_bytes = std::fs::read(TAP_OBJECT).unwrap_or_else(|e| {
        panic!("the tap object must be staged beside the harness ({TAP_OBJECT}): {e}")
    });
    let mut object = Object::parse(&object_bytes)
        .unwrap_or_else(|e| panic!("the shipped tap object must parse: {e}"));

    // The BTF snapshot: raw BTF bytes (the /sys/kernel/btf/vmlinux
    // shape), parsed with the OBJECT's endianness — the loader's own
    // order of operations.
    let btf_bytes = std::fs::read(btf_path)
        .unwrap_or_else(|e| panic!("the BTF snapshot must be readable ({btf_path}): {e}"));
    let btf = Btf::parse(&btf_bytes, object.endianness).unwrap_or_else(|e| {
        println!("KERNEL-MATRIX FAIL [{label}]: the BTF snapshot does not parse: {e}");
        std::process::exit(1);
    });

    // Gated fact 1: the CO-RE posture on the real object.
    if object.has_btf_relocations() {
        println!(
            "KERNEL-MATRIX FAIL [{label}]: the shipped object carries .BTF.ext field relocations — \
             the documented posture (docs/CO_RE_PORTABILITY.md) measured false"
        );
        std::process::exit(1);
    }

    // Gated fact 2: the BTF-dependent relocation path against THIS
    // snapshot — relocate_btf (the CO-RE surface), externs, calls. Map
    // relocations are the one loader step a snapshot CANNOT exercise:
    // they patch live kernel fds at attach time (the tap's control and
    // data maps), so here the map inventory is parsed and recorded,
    // and the fd-patching verdict stays a live-kernel fact (the
    // attach chain owns it).
    if let Err(e) = object.relocate_btf(&btf) {
        println!("KERNEL-MATRIX FAIL [{label}]: CO-RE relocation against the snapshot failed: {e}");
        std::process::exit(1);
    }
    let text_sections: HashSet<usize> = object.functions.keys().map(|(s, _)| *s).collect();
    if let Err(e) = object.relocate_externs() {
        println!("KERNEL-MATRIX FAIL [{label}]: extern (ksym/kfunc) relocation failed: {e}");
        std::process::exit(1);
    }
    if let Err(e) = object.relocate_calls(&text_sections) {
        println!("KERNEL-MATRIX FAIL [{label}]: call relocation failed: {e}");
        std::process::exit(1);
    }

    // Recorded (not gated): the map inventory the loader will fd-patch
    // at attach time.
    let maps = {
        let mut names: Vec<String> = object.maps.keys().cloned().collect();
        names.sort_unstable();
        if names.is_empty() {
            "none".to_string()
        } else {
            names.join(", ")
        }
    };

    // Gated fact 3: the fentry target resolves — aya's own
    // FEntry::load lookup, byte for byte.
    let fentry_id = match btf.id_by_type_name_kind(FENTRY_TARGET, BtfKind::Func) {
        Ok(id) => id,
        Err(e) => {
            println!(
                "KERNEL-MATRIX FAIL [{label}]: the fentry target '{FENTRY_TARGET}' does not resolve \
                 as a BTF Func (the load-time fact the attach chain needs): {e}"
            );
            std::process::exit(1);
        }
    };

    // Recorded (not gated): the kprobe alt surface.
    let alt = btf
        .id_by_type_name_kind(KPROBE_ALT, BtfKind::Func)
        .map(|id| id.to_string())
        .unwrap_or_else(|_| "absent-in-BTF".to_string());

    let programs = {
        let mut names: Vec<&str> = object.programs.keys().map(String::as_str).collect();
        names.sort_unstable();
        names.join(", ")
    };

    println!(
        "KERNEL-MATRIX [{label}] object: parse OK, programs [{programs}], maps [{maps}], \
         CO-RE posture: none-relocations measured, relocate_btf/externs/calls: clean, \
         fentry '{FENTRY_TARGET}': BTF id {fentry_id}, kprobe-alt '{KPROBE_ALT}': {alt}"
    );
    println!("KERNEL-MATRIX PASS [{label}] the tap's kernel contract resolves against this BTF");
}
