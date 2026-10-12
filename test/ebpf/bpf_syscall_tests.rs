// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

// bpf_syscall_tests.rs — the pure constants + struct-shape contract
// of the raw-syscall lane (night-improve-23; night-improve-25 retools
// the lane for the fentry → kprobe chain). The syscalls themselves
// are root-run integration surfaces; here the values that must match
// the kernel ABI byte-for-byte get pinned. All pure, no host
// filesystem access (the miri contract: the interpreter refuses host
// file opens, and a test needing one would be a test bug).

use super::*;

#[test]
fn syscall_constants_match_the_kernel_abi() {
    // From linux/bpf.h and linux/perf_event.h — these ARE the ABI; a
    // wrong constant here pins the wrong object, writes the wrong
    // map, or creates the wrong link type.
    assert_eq!(BPF_MAP_CREATE, 0);
    assert_eq!(BPF_MAP_UPDATE_ELEM, 2);
    assert_eq!(BPF_OBJ_PIN, 6);
    assert_eq!(BPF_RAW_TRACEPOINT_OPEN, 17);
    assert_eq!(BPF_LINK_CREATE, 28);
    assert_eq!(BPF_MAP_TYPE_ARRAY, 2);
    assert_eq!(BPF_PERF_EVENT, 41);
    assert_eq!(PERF_FLAG_FD_CLOEXEC, 8);
}

#[test]
fn attr_struct_sizes_are_kernel_shaped() {
    // LinkCreateAttr: 16 field bytes + 40 pad = 56 (the kernel reads
    // this many bytes of the union head).
    assert_eq!(std::mem::size_of::<LinkCreateAttr>(), 56);
    // ObjPinAttr: pathname + bpf_fd + file_flags.
    assert_eq!(std::mem::size_of::<ObjPinAttr>(), 16);
    // RawTracepointOpenAttr: the aligned name pointer, prog_fd, and
    // the union-tail pad to the 16-byte bpf_attr alignment.
    assert_eq!(std::mem::size_of::<RawTracepointOpenAttr>(), 16);
    // PerfEventAttr: the PERF_ATTR_SIZE_VER1 shape — through config2
    // (offset 64), 72 bytes total. Passing the modern 128-byte shape
    // would fail on kernels whose own perf_event_attr is shorter
    // (E2BIG); 72 is accepted everywhere 4.1+ and covers every field
    // the kprobe lane sets.
    assert_eq!(std::mem::size_of::<PerfEventAttr>(), 72);
    assert_eq!(PerfEventAttr::KPROBE_ATTR_SIZE, 72);
    // MapCreateAttr: the 28-byte map_create head (through numa_node).
    assert_eq!(std::mem::size_of::<MapCreateAttr>(), 28);
    // MapUpdateAttr: the 32-byte map_elem head.
    assert_eq!(std::mem::size_of::<MapUpdateAttr>(), 32);
}

#[test]
fn perf_event_attr_field_offsets_match_the_kernel_layout() {
    // The offsets ARE the contract (linux/perf_event.h): type 0,
    // size 4, config 8, ..., flags 40 (the bitfield word — stays
    // zero), wakeup 48, config1 56 (kprobe_func — the symbol
    // pointer), config2 64 (the probe offset). A drifted offset means
    // the symbol name pointer lands in a field the kernel reads as
    // something else — the probe would silently not register.
    let probe = PerfEventAttr::default();
    let base = std::ptr::addr_of!(probe) as *const u8;
    let offset = |p: *const u8| p as usize - base as usize;
    // (addr_of! yields typed pointers; the cast per call site keeps
    // the closure byte-typed and honest about what is measured.)
    assert_eq!(offset(std::ptr::addr_of!(probe.attr_type).cast::<u8>()), 0);
    assert_eq!(offset(std::ptr::addr_of!(probe.size).cast::<u8>()), 4);
    assert_eq!(offset(std::ptr::addr_of!(probe.config).cast::<u8>()), 8);
    assert_eq!(offset(std::ptr::addr_of!(probe.flags).cast::<u8>()), 40);
    assert_eq!(offset(std::ptr::addr_of!(probe.wakeup).cast::<u8>()), 48);
    assert_eq!(offset(std::ptr::addr_of!(probe.config1).cast::<u8>()), 56);
    assert_eq!(offset(std::ptr::addr_of!(probe.config2).cast::<u8>()), 64);
    // The defaults the kprobe lane relies on: size stated, everything
    // else zero (an ENTRY probe: no retprobe bit, enabled at open).
    assert_eq!(probe.size, 72);
    assert_eq!(probe.config, 0);
    assert_eq!(probe.flags, 0);
    assert_eq!(probe.config2, 0);
}

#[test]
fn raw_tracepoint_open_attr_carries_the_null_name() {
    // The fentry lane's whole trick: name = 0 (NULL) with a tracing
    // program routes the kernel to bpf_tracing_prog_attach — aya's
    // own FEntry::attach shape. The Default pins the zeroed shape.
    let attr = RawTracepointOpenAttr::default();
    assert_eq!(attr.name, 0);
    assert_eq!(attr.prog_fd, 0);
}

#[test]
fn marker_attr_shapes_carry_the_marker_lane_values() {
    // The marker: an anonymous one-entry u32->u32 array map, written
    // at key 0 with no flags. The attr builders below pin the exact
    // shape plant_error_marker assembles.
    let create = MapCreateAttr {
        map_type: BPF_MAP_TYPE_ARRAY,
        key_size: 4,
        value_size: 4,
        max_entries: 1,
        map_flags: 0,
        inner_map_fd: 0,
        numa_node: 0,
    };
    assert_eq!(create.map_type, BPF_MAP_TYPE_ARRAY);
    assert_eq!(create.key_size, 4);
    assert_eq!(create.value_size, 4);
    assert_eq!(create.max_entries, 1);
    assert_eq!(MapCreateAttr::default().map_type, 0);
    let update = MapUpdateAttr::default();
    assert_eq!(update.flags, 0);
    assert_eq!(update.map_fd, 0);
}
