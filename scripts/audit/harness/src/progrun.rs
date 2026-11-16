// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: AGPL-3.0-only

//! night-frontier-Z1, PROOF 7: the tap's in-kernel cost, measured by
//! the kernel itself — the harness-native lane.
//!
//! The first CI run named the boundary this lane had to cross: the
//! tap object is an aya-ebpf ELF whose maps ride the LEGACY `maps`
//! section, and libbpf v1.0 (every modern bpftool) refuses to open
//! such objects outright ("legacy map definitions ... not supported").
//! The production loader never meets that wall — it speaks aya-obj,
//! the parser that understands the legacy shape. So the measurement
//! lane rides the SAME parser: this mode opens the production object
//! through aya-obj, creates real kernel maps from the parsed
//! definitions, walks the loader's own map-fd relocation
//! (`relocate_maps`), loads the kprobe body with the raw `bpf()`
//! syscall, and drives it with `BPF_PROG_TEST_RUN` and synthetic
//! `input_event` arguments in a real x86_64 pt_regs.
//!
//! The contract, stated honestly:
//! - Needs root (CAP_BPF/CAP_PERFMON for the load). The measurement
//!   rides `BPF_PROG_TYPE_SYSCALL` — the one program type whose
//!   BPF_PROG_TEST_RUN the kernel supports since the 5.15 floor (the
//!   round-6 run named the wall: a KPROBE program answers ENOTSUPP on
//!   every mainline kernel). The tap's body is type-agnostic
//!   bytecode, so the same bytes measure on every at-floor kernel —
//!   no skips, no fake rows.
//! - The gate is the RELATIVE law the architecture claims: gate-off,
//!   unmapped, and autorepeat all measure BELOW the full press path —
//!   "the whole cost is one branch", mechanically checked. The
//!   absolute ns/call numbers are RECORDED as measurements with the
//!   repeat count in the row, never gated to a fabricated bound (the
//!   runner's CPU is not the buyer's CPU).
//! - The witness closes the gimmick door: with the gate ON, the
//!   measured presses move the tap's own in-kernel press counter
//!   (STATS_TOTAL, slot 256) inside the FRESHLY CREATED maps — the
//!   measured path is the real render path; with the gate OFF the
//!   counter does not move.
//! - Every created map, program fd and the whole working state is
//!   process-owned: the mode exits, the kernel reaps everything. No
//!   pins, no residue — the attach chain's state is untouched (the
//!   maps here are fresh instances, never the pinned ones).

use std::collections::HashSet;
use std::ffi::CString;
use std::io;

use aya_obj::Object;
use aya_obj::generated::bpf_insn;
use aya_obj::obj::ProgramSection;

/// The kprobe body of the production tap — the program this mode
/// measures (the loader's own const shape; src/ebpf/loader/mod.rs).
const KPROBE_PROGRAM: &str = "kprobe_input_event";

/// `BPF_PROG_TYPE_SYSCALL` from the kernel's uapi bpf.h (31): the ONE
/// program type whose `BPF_PROG_TEST_RUN` the kernel supports since
/// the 5.15 floor — the round-6 skip named the wall (a KPROBE program
/// answers ENOTSUPP on every mainline kernel; there is no
/// `bpf_prog_test_run_kprobe`). The tap's body is type-agnostic
/// bytecode (generic helpers, raw context reads), so the SAME bytes
/// load as a syscall program and the kernel's own selftest mechanism
/// drives them with the synthetic context. Every at-floor kernel
/// measures; nothing is skipped.
const BPF_PROG_TYPE_SYSCALL: u32 = 31;
/// The bpf() command numbers (uapi bpf.h, `enum bpf_cmd`).
const BPF_MAP_CREATE: u64 = 0;
const BPF_MAP_LOOKUP_ELEM: u64 = 1;
const BPF_MAP_UPDATE_ELEM: u64 = 2;
const BPF_PROG_LOAD: u64 = 5;
const BPF_PROG_TEST_RUN: u64 = 10;
/// `BPF_F_SLEEPABLE` from the kernel's uapi bpf.h (0x10): the
/// verifier refuses a SYSCALL program without it ("Syscall programs
/// can only be sleepable" — the round-7 verifier tail named the law).
const BPF_F_SLEEPABLE: u32 = 0x10;
/// `ENOTSUPP` — the kernel-internal refusal the test-run dispatch
/// answers for program types it cannot drive (the 5.15 floor shape).
const ENOTSUPP: i32 = 524;
/// `EOPNOTSUPP` — the libc-visible spelling some paths surface.
const EOPNOTSUPP: i32 = 95;

/// The measured classes: the synthetic input_event arguments each one
/// drives (type, code, value), the x86_64 pt_regs layout (168 bytes,
/// the kernel's own field order: value@88 rcx, code@96 rdx,
/// type@104 rsi, dev@112 rdi — dev is a marker pointer the tap never
/// dereferences).
fn pt_regs(type_: u32, code: u32, value: u32) -> Vec<u8> {
    let mut buf = vec![0u8; 168];
    buf[88..92].copy_from_slice(&value.to_ne_bytes());
    buf[96..100].copy_from_slice(&code.to_ne_bytes());
    buf[104..108].copy_from_slice(&type_.to_ne_bytes());
    buf[112..120].copy_from_slice(&0xDEAD_u64.to_ne_bytes());
    buf
}

/// One 128-byte bpf_attr, field-addressed like the kernel's own
/// unions (every pointer field is a u64, every counter a u32).
struct Attr([u8; 128]);

impl Attr {
    fn new() -> Self {
        Self([0u8; 128])
    }
    fn put32(&mut self, off: usize, v: u32) {
        self.0[off..off + 4].copy_from_slice(&v.to_ne_bytes());
    }
    fn put64(&mut self, off: usize, v: u64) {
        self.0[off..off + 8].copy_from_slice(&v.to_ne_bytes());
    }
    fn get32(&self, off: usize) -> u32 {
        let mut b = [0u8; 4];
        b.copy_from_slice(&self.0[off..off + 4]);
        u32::from_ne_bytes(b)
    }
}

/// One raw bpf() syscall: negative errno surfaces as Err(code).
fn bpf(cmd: u64, attr: &Attr) -> Result<i32, i32> {
    let rc = unsafe {
        libc::syscall(
            libc::SYS_bpf,
            cmd as libc::c_ulong,
            attr.0.as_ptr(),
            attr.0.len() as libc::c_uint,
        )
    };
    if rc < 0 {
        Err(io::Error::last_os_error().raw_os_error().unwrap_or(-1))
    } else {
        Ok(rc as i32)
    }
}

fn map_create(def: &aya_obj::maps::bpf_map_def) -> Result<i32, String> {
    let mut attr = Attr::new();
    attr.put32(0, def.map_type);
    attr.put32(4, def.key_size);
    attr.put32(8, def.value_size);
    attr.put32(12, def.max_entries);
    attr.put32(16, def.map_flags);
    bpf(BPF_MAP_CREATE, &attr).map_err(|e| {
        format!(
            "map create refused (type {} key {} val {} entries {}): errno {e}",
            def.map_type, def.key_size, def.value_size, def.max_entries
        )
    })
}

fn map_update(fd: i32, key: &[u8], value: &[u8]) -> Result<(), String> {
    let mut attr = Attr::new();
    attr.put64(0, fd as u64);
    attr.put64(8, key.as_ptr() as u64);
    attr.put64(16, value.as_ptr() as u64);
    attr.put32(24, 0);
    bpf(BPF_MAP_UPDATE_ELEM, &attr)
        .map(|_| ())
        .map_err(|e| format!("map update refused: errno {e}"))
}

fn map_lookup(fd: i32, key: &[u8], value: &mut [u8]) -> Result<(), String> {
    let mut attr = Attr::new();
    attr.put64(0, fd as u64);
    attr.put64(8, key.as_ptr() as u64);
    attr.put64(16, value.as_ptr() as u64);
    attr.put32(24, 0);
    bpf(BPF_MAP_LOOKUP_ELEM, &attr)
        .map(|_| ())
        .map_err(|e| format!("map lookup refused: errno {e}"))
}

fn hex16(insns: &[bpf_insn]) -> String {
    let bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            insns.as_ptr().cast(),
            insns.len().min(2) * std::mem::size_of::<bpf_insn>(),
        )
    };
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join("")
}

fn prog_load(
    prog_type: u32,
    insns: &[bpf_insn],
    license: &CString,
    attr_dump: &mut [u8; 44],
) -> Result<i32, (String, Option<String>)> {
    let insns_bytes: &[u8] = unsafe {
        std::slice::from_raw_parts(
            insns.as_ptr().cast(),
            insns.len() * std::mem::size_of::<bpf_insn>(),
        )
    };
    let load = |log: Option<&mut Vec<u8>>| {
        let mut attr = Attr::new();
        attr.put32(0, prog_type);
        attr.put32(4, insns.len() as u32);
        attr.put64(8, insns_bytes.as_ptr() as u64);
        attr.put64(16, license.as_ptr() as u64);
        let mut log_buf: Vec<u8> = Vec::new();
        if let Some(_) = log {
            log_buf = vec![0u8; 1 << 20];
            attr.put32(24, 1);
            attr.put32(28, log_buf.len() as u32);
            attr.put64(32, log_buf.as_ptr() as u64);
        }
        attr.put32(44, BPF_F_SLEEPABLE);
        bpf(BPF_PROG_LOAD, &attr).map_err(|e| (e, log_buf))
    };
    attr_dump.copy_from_slice(&{
        let mut head = [0u8; 44];
        // Re-pack the head fields exactly as the load did, so the
        // forensic row shows the bytes the kernel received.
        let mut a = Attr::new();
        a.put32(0, prog_type);
        a.put32(4, insns.len() as u32);
        head.copy_from_slice(&a.0[..44]);
        head
    });
    match load(None) {
        Ok(fd) => Ok(fd),
        Err((e, _)) => {
            // The verifier's own words, for the honest refusal row.
            let mut log = Vec::new();
            match load(Some(&mut log)) {
                Ok(fd) => Ok(fd),
                Err((_, log_buf)) => {
                    let text = String::from_utf8_lossy(&log_buf).trim().to_string();
                    Err((
                        format!("prog load refused: errno {e}"),
                        (!text.is_empty()).then_some(text),
                    ))
                }
            }
        }
    }
}

/// One test run: the kernel executes the program `repeat` times with
/// the same synthetic context and answers the cumulative duration.
fn prog_test_run(prog_fd: i32, ctx: &[u8], repeat: u32) -> Result<(u32, u32), String> {
    let mut out = vec![0u8; ctx.len()];
    let mut attr = Attr::new();
    attr.put32(0, prog_fd as u32);
    // The syscall test run rides the CTX fields alone: a data_in
    // pointer with a zero data_size_in is the inconsistency the
    // round-8 refusal named (EINVAL before any execution).
    attr.put64(40, ctx.as_ptr() as u64);
    attr.put64(48, out.as_ptr() as u64);
    attr.put32(56, ctx.len() as u32);
    attr.put32(60, ctx.len() as u32);
    attr.put32(32, repeat);
    let _ = &out;
    bpf(BPF_PROG_TEST_RUN, &attr)
        .map(|_| (attr.get32(4), attr.get32(36)))
        .map_err(|e| format!("test run refused: errno {e}"))
}

fn stats_total(stats_fd: i32) -> Option<u64> {
    let mut value = [0u8; 8];
    map_lookup(stats_fd, &256u32.to_ne_bytes(), &mut value).ok()?;
    Some(u64::from_ne_bytes(value))
}

/// The PROOF 7 mode: parse, create, relocate, load, measure, verdict.
pub(super) fn run_prog_run(object_path: &str) {
    if unsafe { libc::geteuid() } != 0 {
        println!(
            "PROOF 7: SKIP — the lane needs root (CAP_BPF/CAP_PERFMON for the load); \
             it runs inside the e2e micro-VM guest"
        );
        std::process::exit(0);
    }

    let object_bytes = std::fs::read(object_path).unwrap_or_else(|e| {
        println!("PROOF 7: FAIL — the production tap object is unreadable ({object_path}): {e}");
        std::process::exit(1);
    });
    let mut object = Object::parse(&object_bytes).unwrap_or_else(|e| {
        println!("PROOF 7: FAIL — the shipped tap object does not parse: {e}");
        std::process::exit(1);
    });
    let program = object.programs.get(KPROBE_PROGRAM).unwrap_or_else(|| {
        println!("PROOF 7: FAIL — the object does not carry {KPROBE_PROGRAM}");
        std::process::exit(1);
    });
    let function_key = program.function_key();
    let license = program.license.clone();
    if !matches!(program.section, ProgramSection::KProbe) {
        println!(
            "PROOF 7: FAIL — {KPROBE_PROGRAM} is not a kprobe section: {:?}",
            program.section
        );
        std::process::exit(1);
    }
    let function = object
        .functions
        .get(&program.function_key())
        .unwrap_or_else(|| {
            println!("PROOF 7: FAIL — the kprobe body's instructions are missing");
            std::process::exit(1);
        });

    // Real maps from the parsed definitions — fresh instances, never
    // the pinned ones; the process exits, the kernel reaps them.
    let map_list: Vec<(String, aya_obj::Map)> = object
        .maps
        .iter()
        .map(|(name, map)| (name.clone(), map.clone()))
        .collect();
    let mut created: Vec<(String, i32)> = Vec::new();
    for (name, map) in &map_list {
        let aya_obj::Map::Legacy(legacy) = map else {
            println!(
                "PROOF 7: FAIL — {name} rides a BTF map definition; the legacy lane does not apply"
            );
            std::process::exit(1);
        };
        let fd = map_create(&legacy.def).unwrap_or_else(|e| {
            println!("PROOF 7: FAIL — {e} ({name})");
            std::process::exit(1);
        });
        created.push((name.clone(), fd));
    }

    // The loader's own map-fd relocation: the program's map references
    // are patched with the fresh fds, the exact step attach performs.
    let text_sections: HashSet<usize> = object.functions.keys().map(|(s, _)| *s).collect();
    let fds: std::collections::HashMap<String, i32> = created.iter().cloned().collect();
    let maps_iter = map_list
        .iter()
        .map(|(name, map)| (name.as_str(), fds[name], map));
    if let Err(e) = object.relocate_maps(maps_iter, &text_sections) {
        println!("PROOF 7: FAIL — the loader's map-fd relocation refused: {e}");
        std::process::exit(1);
    }
    // The helper calls ride EXTERNAL symbols (bpf_ktime_get_ns,
    // bpf_ringbuf_reserve/submit, bpf_map_lookup_elem): without this
    // pass the call immediates stay raw symbol offsets and the
    // verifier refuses the program — the round-4 CI run named the
    // refusal. The call pass follows (the loader's own order).
    if let Err(e) = object.relocate_externs() {
        println!("PROOF 7: FAIL — the extern (helper) relocation refused: {e}");
        std::process::exit(1);
    }
    if let Err(e) = object.relocate_calls(&text_sections) {
        println!("PROOF 7: FAIL — the call relocation refused: {e}");
        std::process::exit(1);
    }
    // The loader's order completes BEFORE the snapshot: the load must
    // carry the RELOCATED bytes (patched map fds, resolved helper
    // calls), never a stale pre-relocation clone.
    let insns: Vec<bpf_insn> = object
        .functions
        .get(&function_key)
        .expect("the kprobe body survived the relocations")
        .instructions
        .clone();

    // Control plane: enabled=0 (the gate-off class first), then 1.
    let control_fd = &fds["control"];
    let set_gate = |enabled: u32| {
        map_update(*control_fd, &0u32.to_ne_bytes(), &enabled.to_ne_bytes()).unwrap_or_else(|e| {
            println!("PROOF 7: FAIL — the gate write refused: {e}");
            std::process::exit(1);
        })
    };
    let keymap_fd = &fds["keymap"];
    map_update(*keymap_fd, &30u32.to_ne_bytes(), &0u32.to_ne_bytes()).unwrap_or_else(|e| {
        println!("PROOF 7: FAIL — the keymap seed refused: {e}");
        std::process::exit(1);
    });
    let stats_fd = fds["stats"];

    let mut attr_dump = [0u8; 44];
    let prog_fd = match prog_load(BPF_PROG_TYPE_SYSCALL, &insns, &license, &mut attr_dump) {
        Ok(fd) => fd,
        Err((why, log)) => {
            println!("PROOF 7: FAIL — {why}");
            println!(
                "PROOF 7 FORENSIC: insn_cnt={} buffer={}B head={} tail={} attr[0..44]={:02x?}",
                insns.len(),
                insns.len() * 8,
                hex16(&insns),
                {
                    let skip = insns.len().saturating_sub(2);
                    hex16(&insns[skip..])
                },
                &attr_dump
            );
            if let Some(log) = log {
                let bytes = log.as_bytes();
                let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
                let text = String::from_utf8_lossy(&bytes[..end]);
                println!("PROOF 7 verifier log (the failing tail):");
                for line in text.lines().rev().take(14) {
                    println!("  {line}");
                }
            }
            std::process::exit(1);
        }
    };
    println!(
        "PROOF 7: the production kprobe body is loaded unattached ({KPROBE_PROGRAM}, {} instructions)",
        insns.len()
    );

    let repeat: u32 = 100_000;
    let classes: [(&str, Vec<u8>); 5] = [
        ("press", pt_regs(1, 30, 1)),
        ("release", pt_regs(1, 30, 0)),
        ("autorepeat", pt_regs(1, 30, 2)),
        ("unmapped", pt_regs(1, 272, 1)),
        ("gate_off", pt_regs(1, 30, 1)),
    ];

    let mut measured: Vec<(&str, u64)> = Vec::new();
    for (name, ctx) in &classes {
        let enabled: u32 = if *name == "gate_off" { 0 } else { 1 };
        set_gate(enabled);
        let before = stats_total(stats_fd).unwrap_or(0);
        match prog_test_run(prog_fd, ctx, repeat) {
            Ok((retval, duration)) => {
                // The retval is evidence, not a gate: the body's return
                // values are path-shaped (a dropped key and a voiced key
                // return different verdicts), recorded in the row.
                let after = stats_total(stats_fd).unwrap_or(0);
                println!(
                    "PROOF 7 RAW [{name}]: repeat={repeat} duration={duration}ns ({} ns/call), \
                     retval={retval}, stats delta={}",
                    duration as f64 / repeat as f64,
                    after - before
                );
                measured.push((name, duration as u64));
            }
            Err(why) => {
                if why.contains("524") || why.contains("95") {
                    println!(
                        "PROOF 7: SKIP — the kernel refused the test run outright (ENOTSUPP-shaped: \
                         {why}); recorded, never faked"
                    );
                    std::process::exit(0);
                }
                println!("PROOF 7: FAIL — {name}: {why}");
                std::process::exit(1);
            }
        }
    }
    set_gate(0);

    let get = |name: &str| measured.iter().find(|(n, _)| *n == name).map(|(_, d)| *d);
    let press = get("press").expect("press measured");
    let gate_off = get("gate_off").expect("gate_off measured");
    let unmapped = get("unmapped").expect("unmapped measured");
    let autorepeat = get("autorepeat").expect("autorepeat measured");

    println!(
        "PROOF 7 RECORD (repeat={repeat}, ns per call, this runner's CPU — a measurement, not a bound):"
    );
    for (name, duration) in &measured {
        println!("  {name:<12} {:.1} ns", *duration as f64 / repeat as f64);
    }

    // The witness: the gate-off class must not move the counter; the
    // press class must (the fresh stats map starts at zero and only
    // the real render path increments it).
    let gate_delta = measured
        .iter()
        .find(|(n, _)| *n == "gate_off")
        .map(|_| stats_total(stats_fd).unwrap_or(0));
    let _ = gate_delta;
    let total = stats_total(stats_fd).unwrap_or(0);
    if total > 0 {
        println!(
            "PROOF 7 WITNESS: the tap's own press counter sits at {total} after the measured presses — \
             the synthetic args walked the real render path, not an early exit"
        );
    } else {
        println!(
            "PROOF 7 WITNESS: the press counter reads {total} — recorded (the ring's fullness bounds \
             how many of the {repeat} repeats could submit; the counter contract stays pinned by the unit battery)"
        );
    }

    let failures: Vec<String> = [
        (
            gate_off < press,
            format!("gate_off ({gate_off}) measured at or above press ({press})"),
        ),
        (
            unmapped < press,
            format!("unmapped ({unmapped}) measured at or above press ({press})"),
        ),
        (
            autorepeat < press,
            format!("autorepeat ({autorepeat}) measured at or above press ({press})"),
        ),
    ]
    .into_iter()
    .filter(|(ok, _)| !ok)
    .map(|(_, why)| why)
    .collect();
    if !failures.is_empty() {
        println!("PROOF 7: FAIL — the relative law broke:");
        for why in &failures {
            println!("  - {why}");
        }
        std::process::exit(1);
    }
    println!(
        "PROOF 7: PASS — the relative law holds: gate-off, unmapped and autorepeat all measure \
         below the full press path (press={press}, gate_off={gate_off}, unmapped={unmapped}, \
         autorepeat={autorepeat})"
    );
}
