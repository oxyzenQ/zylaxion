// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The eBPF object staging machinery (night-improve-23), split out of
//! build.rs per the 600-LOC no-mercy cap (the zelynic build/ module
//! precedent: flags/preflight/validate live beside their build.rs too).
//! Called from build.rs's main(); everything here is build-script-only.

use std::process::Command;

/// The dated nightly pin driving the nested cross-build — mirrors
/// ebpf/rust-toolchain.toml (which the nested build resolves from its own
/// directory) and the pin scripts/dev/bootstrap-ebpf.sh installs on the
/// host. The pin and the bpf-linker release are a validated pair (the
/// zelynic lineage pairing, carried over unchanged).
const EBPF_TOOLCHAIN: &str = "nightly-2026-09-18";

/// The pinned bpf-linker release the nested build links with — the second
/// half of the validated pair. The preflight names this version in its
/// fix message so a drifted host install is diagnosable in one read.
const BPF_LINKER_VERSION: &str = "0.11.1";

/// The one production BPF object (night-improve-23): the tap. The
/// staged name is the registry lane's constant - one name, two lanes.
const TAP_OBJECT: &str = crate::prebuilt::STAGED_OBJECT_NAME;

// ── night-improve-23: the eBPF object staging lanes ─────────────────────
//
// Three lanes produce (or skip) the embedded object, all landing at the
// same OUT_DIR name the loader embeds (`zylaxion-tap`):
//
//   dormant           — the `ebpf` feature is off: nothing stages, the
//                       loader module is not compiled either.
//   registry-prebuilt — a registry tarball (cargo's package walk cannot
//                       carry nested packages, so no ebpf/ tree rides the
//                       extract): the maintainer-built object tracked in
//                       ebpf-prebuilt/ is staged from build/prebuilt.rs,
//                       manifest-verified (the object blake3 must match
//                       the ebpf-prebuilt/manifest.toml pin before the
//                       bytes are embedded; provenance in the manifest).
//                       This is the lane that lets a plain
//                       `cargo install zylaxion` land the full-featured
//                       binary on a stable toolchain.
//   source-built      — a git checkout: the ebpf/ crate is cross-compiled
//                       by a nested cargo invocation under the dated
//                       nightly pin, and the artifact is staged after
//                       structural validation (with one self-heal retry).
//
// night-dinner-4 adds the lane's self-heal half: after a source-built
// staging succeeds, refresh_prebuilt_lane consults the generator
// script's --check verdict and, when ebpf-prebuilt/ lags the ebpf/
// tree (or its staged copy no longer matches its own pins),
// regenerates it with no human in the loop
// — the manual "remember to regenerate" step is retired, and the
// beta.1 incident shape (an ebpf/ commit riding a stale manifest pin)
// cannot be produced by a build-then-commit flow anymore. The script
// stays the one writer; the parity gate stays the commit-time
// enforcement.
//
// The source-built lane keeps the zelynic hardening that earned its
// place: the prerequisite preflight (fail before compile time with the
// one-command fix), the RUSTFLAGS poison strip plus the v3 force (env
// rustflags shadow config; host-CPU flags are poison for bpfel), and
// the structural ELF validation with the damaged-artifact self-heal
// (cargo's freshness is fingerprint-plus-existence, never output
// integrity — a truncated object would ride every later build).

pub(super) fn build_ebpf_objects() {
    let manifest_dir =
        std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let ebpf_dir = manifest_dir.join("ebpf");

    // Rerun triggers: the ebpf crate's sources and its build wiring.
    // ebpf/target is deliberately NOT watched — the nested cargo owns
    // its own freshness, and watching build output would loop.
    for rel in [
        "src",
        "Cargo.toml",
        "Cargo.lock",
        ".cargo/config.toml",
        "rust-toolchain.toml",
    ] {
        println!("cargo:rerun-if-changed={}", ebpf_dir.join(rel).display());
    }

    // Only the ebpf feature needs the object.
    if std::env::var_os("CARGO_FEATURE_EBPF").is_none() {
        println!("cargo:rustc-env=ZYLAXION_EBPF_LANE=dormant (not compiled)");
        return;
    }

    // Registry lane: no ebpf/ tree rides a crates.io extract. The
    // staging (and the manifest verification that guards it) lives in
    // build/prebuilt.rs - the same split the zejtron reference runs.
    if !ebpf_dir.join("Cargo.toml").is_file() {
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
        crate::prebuilt::stage_prebuilt_object(&manifest_dir, &out_dir);
        println!("cargo:rustc-env=ZYLAXION_EBPF_LANE=registry-prebuilt");
        return;
    }

    // Preflight BEFORE the nested build: name the missing prerequisite
    // and its one-command fix, and catch a missing bpf-linker even when
    // ebpf/target is fully cached (a warm build skips the link step).
    preflight_ebpf_prerequisites(EBPF_TOOLCHAIN);

    run_nested_ebpf_build(&ebpf_dir);

    // Stage with structural validation; on damage, self-heal: delete
    // every on-disk copy and force one rebuild, then panic if the
    // object is damaged AGAIN (that is a host or toolchain problem,
    // not a stale-freshness problem).
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let release_dir = ebpf_dir
        .join("target")
        .join("bpfel-unknown-none")
        .join("release");
    match read_validated_ebpf_object(&release_dir.join(TAP_OBJECT)) {
        Ok(data) => {
            stage_object(&out_dir, &data);
        }
        Err(problem) => {
            println!(
                "cargo:warning=eBPF object {TAP_OBJECT} failed validation ({problem}) — \
                 forcing one rebuild of it"
            );
            delete_damaged_artifact(&release_dir, TAP_OBJECT);
            run_nested_ebpf_build(&ebpf_dir);
            let data = read_validated_ebpf_object(&release_dir.join(TAP_OBJECT)).unwrap_or_else(
                |second_problem| {
                    panic!(
                        "eBPF object {TAP_OBJECT} is damaged again immediately after a \
                         forced rebuild — before: {problem}; after: {second_problem}. \
                         Either bpf-linker is producing a broken object (reinstall the \
                         pin: rustup toolchain install {EBPF_TOOLCHAIN} and the \
                         bpf-linker {BPF_LINKER_VERSION} prebuilt on PATH), or something \
                         on this host is modifying build outputs as they land (indexer, \
                         antivirus, full disk)"
                    )
                },
            );
            stage_object(&out_dir, &data);
        }
    }

    refresh_prebuilt_lane(&manifest_dir);

    println!("cargo:rustc-env=ZYLAXION_EBPF_LANE=source-built");
}

/// night-dinner-4: the automatic ebpf-prebuilt/ refresh. Any edit under
/// ebpf/ (a version bump, a comment, a lock) stales the manifest's
/// tree pin; this runs the generator script's --check verdict after
/// every successful source-built staging and, on staleness, lets the
/// script regenerate the lane immediately — so the next `git status`
/// shows the refreshed manifest.toml and zylaxion-tap to commit
/// alongside the change that caused them.
///
/// Host-independent by the same law the parity gate records: the
/// freshly built object's bytes are never compared against the pins
/// (byte reproducibility is a host property — a foreign rebuild must
/// not churn the lane). Staleness is the tree pin and the staged
/// copy's integrity; when the tree moved, the regenerate flow below
/// installs this host's fresh object as the new pinned bytes.
///
/// Best-effort by law: a refresh that cannot run (sparse checkout
/// without scripts/, a read-only tree) degrades to a cargo:warning
/// naming the manual command — the build itself must not fail for a
/// meta-lane refresh, because scripts/gates/check-prebuilt-parity.sh
/// remains the hard stop at commit time. In-sync builds stay silent:
/// the verdict runs on every build but speaks only when it wrote.
/// ZYLAXION_SKIP_PREBUILT_REGEN=1 is the shared escape hatch (the
/// commit gate honors the same variable): under it the verdict still
/// runs, but the heal is skipped and the warning names the manual
/// command instead.
pub(super) fn refresh_prebuilt_lane(manifest_dir: &std::path::Path) {
    let script = manifest_dir
        .join("scripts")
        .join("dev")
        .join("regenerate-ebpf-prebuilt.sh");
    let verdict = match Command::new(&script)
        .arg("--check")
        .current_dir(manifest_dir)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            println!(
                "cargo:warning=ebpf-prebuilt/ staleness check unavailable ({e}) — before \
                 committing, run: ./scripts/dev/regenerate-ebpf-prebuilt.sh (the parity \
                 gate scripts/gates/check-prebuilt-parity.sh still enforces the pin)"
            );
            return;
        }
    };
    if verdict.status.success() {
        return; // in sync — silent, this is the every-build steady state
    }
    if std::env::var_os("ZYLAXION_SKIP_PREBUILT_REGEN").is_some_and(|v| v == "1") {
        println!(
            "cargo:warning=ebpf-prebuilt/ is stale and ZYLAXION_SKIP_PREBUILT_REGEN=1 skips \
             the refresh — run ./scripts/dev/regenerate-ebpf-prebuilt.sh before committing \
             (the parity gate fails the commit otherwise)"
        );
        return;
    }
    match Command::new(&script).current_dir(manifest_dir).output() {
        Ok(r) if r.status.success() => println!(
            "cargo:warning=ebpf-prebuilt/ refreshed automatically: the manifest pins lagged \
             the ebpf/ tree or the freshly built object — commit the refreshed \
             manifest.toml and zylaxion-tap together with the change that staled them"
        ),
        _ => println!(
            "cargo:warning=ebpf-prebuilt/ is stale and the automatic refresh failed — run \
             ./scripts/dev/regenerate-ebpf-prebuilt.sh before committing (the parity gate \
             scripts/gates/check-prebuilt-parity.sh fails the commit otherwise)"
        ),
    }
}

pub(super) fn stage_object(out_dir: &std::path::Path, data: &[u8]) {
    let dst = out_dir.join(TAP_OBJECT);
    std::fs::write(&dst, data)
        .unwrap_or_else(|e| panic!("failed to stage {} into OUT_DIR: {e}", dst.display()));
}

/// Verify the two source-build prerequisites BEFORE the nested build
/// spends compile time: the dated nightly pin (listed in rustup) and the
/// pinned bpf-linker release on PATH. Each failure names its fix.
pub(super) fn preflight_ebpf_prerequisites(toolchain: &str) {
    let listed = Command::new("rustup")
        .args(["toolchain", "list"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    if !listed
        .lines()
        .any(|l| l.starts_with(&format!("{toolchain}-")))
    {
        panic!(
            "the eBPF source-build needs the dated nightly pin {toolchain} (rust-src \
             included) and it is not installed. One command: rustup toolchain install \
             {toolchain} --profile minimal --component rust-src --component rustfmt \
             (or ./scripts/dev/bootstrap-ebpf.sh, which installs it plus bpf-linker)"
        );
    }
    let linker = match Command::new("bpf-linker").arg("--version").output() {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => panic!(
            "the eBPF source-build needs bpf-linker {BPF_LINKER_VERSION} on PATH and \
             rustc cannot invoke it. Install the prebuilt static binary (no sudo, no \
             system LLVM): ./scripts/dev/bootstrap-ebpf.sh does it, or fetch \
             https://github.com/aya-rs/bpf-linker/releases/download/v{BPF_LINKER_VERSION}/bpf-linker-x86_64-unknown-linux-musl.tar.zst \
             and put bpf-linker on PATH"
        ),
    };
    if !linker.contains(BPF_LINKER_VERSION) {
        panic!(
            "bpf-linker on PATH reports '{linker}', expected {BPF_LINKER_VERSION} — \
             the toolchain/linker pair is validated together; install the pinned \
             release (./scripts/dev/bootstrap-ebpf.sh)"
        );
    }
}

/// The nested eBPF cross-build invocation. The toolchain is forced
/// through `rustup run <pin> cargo` (the CARGO env var handed to build
/// scripts points at the resolved stable cargo, which would silently
/// drop the nightly-only -Z flags); cwd inside ebpf/ keeps the crate's
/// own .cargo/config.toml in effect; target and build-std are ALSO
/// passed explicitly so the invocation is correct even without the
/// config. Environment: the parent's RUSTC/RUSTC_WRAPPER are poison for
/// the sub-build (the stable compiler would compile build-std core); the
/// inherited rustflags pass through the host-poison strip and then get
/// the v3 codegen level forced. --locked keeps the committed Cargo.lock
/// authoritative; stdio is inherited so the nested compiler output
/// streams through to the build log.
pub(super) fn run_nested_ebpf_build(ebpf_dir: &std::path::Path) {
    let mut nested = Command::new("rustup");
    nested
        .args(["run", EBPF_TOOLCHAIN, "cargo"])
        .current_dir(ebpf_dir)
        .args([
            "build",
            "--release",
            "--locked",
            "--target",
            "bpfel-unknown-none.json",
        ])
        .args(["-Z", "build-std=core"])
        .args(["-Z", "json-target-spec"])
        .env_remove("RUSTC")
        .env_remove("RUSTC_WORKSPACE_WRAPPER")
        .env_remove("RUSTC_WRAPPER");
    strip_host_poison_rustflags(&mut nested);
    force_bpf_v3_rustflags(&mut nested);
    let status = nested.status().unwrap_or_else(|e| {
        panic!(
            "failed to launch `rustup run {EBPF_TOOLCHAIN} cargo` — rustup is a hard \
             prerequisite of the pure-Rust eBPF build: {e}"
        )
    });
    if !status.success() {
        panic!(
            "the pure-Rust eBPF build failed with prerequisites present ({EBPF_TOOLCHAIN} \
             and bpf-linker {BPF_LINKER_VERSION} were both verified) — the nested \
             compiler output above is the diagnosis"
        );
    }
}

/// Remove every on-disk copy of one damaged eBPF artifact so the nested
/// cargo run that follows is forced to RELINK it. Two locations, both
/// required: cargo's target layout keeps the canonical unit output in
/// build/zylaxion-ebpf/<fingerprint>/out/zylaxion_tap and publishes a
/// HARDLINK at bpfel-unknown-none/release/zylaxion-tap — deleting only
/// the published name changes nothing (the canonical copy declares the
/// unit fresh); a missing CANONICAL copy is what dirties the unit.
pub(super) fn delete_damaged_artifact(release_dir: &std::path::Path, name: &str) {
    let published = release_dir.join(name);
    if let Err(e) = std::fs::remove_file(&published)
        && e.kind() != std::io::ErrorKind::NotFound
    {
        panic!(
            "failed to delete the damaged eBPF object {}: {e} — cannot force a clean \
             rebuild around this",
            published.display()
        );
    }
    let canonical_pkg = release_dir.join("build").join("zylaxion-ebpf");
    let canonical_name = name.replace('-', "_");
    match std::fs::read_dir(&canonical_pkg) {
        Ok(hashes) => {
            for hash in hashes.flatten() {
                let out = hash.path().join("out").join(&canonical_name);
                if let Err(e) = std::fs::remove_file(&out)
                    && e.kind() != std::io::ErrorKind::NotFound
                {
                    panic!(
                        "failed to delete the damaged canonical eBPF artifact {}: {e} — \
                         cannot force a clean rebuild around this",
                        out.display()
                    );
                }
            }
        }
        // No build/ directory — no canonical copies; the published name
        // above was the only copy.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => panic!(
            "failed to scan {} for canonical artifact copies: {e} — cannot force a \
             clean rebuild around this",
            canonical_pkg.display()
        ),
    }
}

/// Read one staged eBPF object and structurally validate it. The Err
/// text is user-facing (it rides the cargo:warning and the self-heal
/// panic), so it names the exact violation instead of a bare "corrupt".
pub(super) fn read_validated_ebpf_object(path: &std::path::Path) -> Result<Vec<u8>, String> {
    let data = std::fs::read(path).map_err(|e| format!("not readable: {e}"))?;
    validate_ebpf_object(&data)?;
    Ok(data)
}

/// Structural validation of one staged eBPF object — enough ELF law to
/// catch every on-disk damage mode that would make aya's parser reject
/// the bytes, with zero build-dependencies: identity (magic, ELFCLASS64,
/// little-endian, version 1), ET_REL + EM_BPF (the only shape bpf-linker
/// emits), and a section header table that exists, uses 64-byte entries,
/// and lies fully inside the file (bpf-linker places it LAST, which makes
/// this the truncation killer). Deliberately not checked: semantic
/// content (programs, maps, relocations) — that is aya's own job at load
/// time.
pub(super) fn validate_ebpf_object(data: &[u8]) -> Result<(), String> {
    if data.len() < 64 {
        return Err(format!(
            "too short to hold an ELF64 header: {} bytes",
            data.len()
        ));
    }
    if !data.starts_with(&[0x7f, b'E', b'L', b'F']) {
        return Err("ELF magic missing (not an ELF file)".to_string());
    }
    if data[4] != 2 {
        return Err(format!("not ELFCLASS64: class {}", data[4]));
    }
    if data[5] != 1 {
        return Err(format!("not little-endian: data encoding {}", data[5]));
    }
    if data[6] != 1 {
        return Err(format!("unknown ELF version {}", data[6]));
    }
    let u16_at = |off: usize| u16::from_le_bytes([data[off], data[off + 1]]);
    let u32_at =
        |off: usize| u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]);
    let u64_at = |off: usize| {
        let mut b = [0u8; 8];
        b.copy_from_slice(&data[off..off + 8]);
        u64::from_le_bytes(b)
    };
    let e_type = u16_at(16);
    if e_type != 1 {
        return Err(format!("not ET_REL (relocatable): type {e_type}"));
    }
    let e_machine = u16_at(18);
    if e_machine != 247 {
        // 247 = EM_BPF — the only machine bpf-linker emits.
        return Err(format!("not EM_BPF: machine {e_machine}"));
    }
    let shoff = u64_at(0x28) as usize;
    let shentsize = u16_at(0x3A) as usize;
    let shnum = u16_at(0x3C) as usize;
    if shentsize != 64 {
        return Err(format!("section entry size is {shentsize}, expected 64"));
    }
    let table_end = shoff
        .checked_add(
            shentsize
                .checked_mul(shnum)
                .ok_or("section table size overflow")?,
        )
        .ok_or("section table end overflow")?;
    if table_end > data.len() {
        return Err(format!(
            "section header table ({shoff}..{table_end}) lies past the file end ({} \
             bytes) — the object is truncated",
            data.len()
        ));
    }
    for i in 0..shnum {
        let base = shoff + i * 64;
        let sh_offset = u64_at(base + 0x18) as usize;
        let sh_size = u64_at(base + 0x20) as usize;
        let sh_type = u32_at(base + 4);
        // NOBITS (8) occupies no file space; everything else must fit.
        if sh_type != 8 {
            let end = sh_offset
                .checked_add(sh_size)
                .ok_or("section end overflow")?;
            if end > data.len() {
                return Err(format!(
                    "section {i} ({sh_offset}..{end}) lies past the file end ({} \
                     bytes) — the object is truncated",
                    data.len()
                ));
            }
        }
    }
    Ok(())
}

/// Strip the host-poison tokens from the inherited rustflags before the
/// nested bpfel build sees them: host-CPU and host-linker flags are
/// poison for a cross-build ("-Ctarget-cpu=native" lowers for the BUILD
/// host's ISA; "-Clinker=..." retargets the linker). Everything else
/// survives verbatim, so CI's `RUSTFLAGS="-D warnings"` contract keeps
/// covering the ebpf crate.
pub(super) fn strip_host_poison_rustflags(cmd: &mut Command) {
    if let Some(value) = std::env::var_os("CARGO_ENCODED_RUSTFLAGS") {
        match strip_host_poison(&value.to_string_lossy(), '\u{1f}') {
            Some(filtered) => {
                cmd.env("CARGO_ENCODED_RUSTFLAGS", filtered);
            }
            None => {
                cmd.env_remove("CARGO_ENCODED_RUSTFLAGS");
            }
        }
    }
    if let Some(value) = std::env::var_os("RUSTFLAGS") {
        match strip_host_poison(&value.to_string_lossy(), ' ') {
            Some(filtered) => {
                cmd.env("RUSTFLAGS", filtered);
            }
            None => {
                cmd.env_remove("RUSTFLAGS");
            }
        }
    }
}

/// The pure token filter behind the poison strip: `Some(joined)` when
/// any token was dropped, `None` when nothing changed (so the caller can
/// env_remove instead of re-setting an identical value).
pub(super) fn strip_host_poison(value: &str, sep: char) -> Option<String> {
    let tokens: Vec<&str> = value
        .split(sep)
        .filter(|t| {
            let t = t.trim();
            !(t.contains("target-cpu")
                || t.contains("linker=")
                || t.contains("link-self-contained"))
        })
        .collect();
    let dropped = tokens.len() != value.split(sep).count();
    if dropped {
        Some(tokens.join(&sep.to_string()))
    } else {
        None
    }
}

/// Force the v3 BPF codegen level onto the nested build, whatever
/// survived the poison strip: the LLVM BPF backend selects the BPF_ATOMIC
/// RMW instructions (the SMP-safe stats counters need them) only at cpu
/// v3. A set env variable SHADOWS config rustflags entirely, so the flag
/// is appended to whichever rustflags variable survived; with neither
/// variable set the ebpf/.cargo/config.toml supplies the same flag.
pub(super) fn force_bpf_v3_rustflags(cmd: &mut Command) {
    const FLAG: &str = "-Ctarget-cpu=v3";
    // An empty-but-SET variable is the same as unset for flag purposes —
    // appending to it would inject an empty token that rustc reads as a
    // phantom filename (reproduced live: `rustc - ... '' -Ctarget-cpu=v3`).
    let encoded = std::env::var_os("CARGO_ENCODED_RUSTFLAGS").filter(|v| !v.is_empty());
    if let Some(value) = encoded {
        let joined = format!("{}\u{1f}{FLAG}", value.to_string_lossy());
        cmd.env("CARGO_ENCODED_RUSTFLAGS", joined);
    } else {
        let plain = std::env::var_os("RUSTFLAGS").filter(|v| !v.is_empty());
        if let Some(value) = plain {
            let joined = format!("{} {FLAG}", value.to_string_lossy());
            cmd.env("RUSTFLAGS", joined);
        }
        // Neither variable set (or both empty): the ebpf/.cargo/config.toml
        // supplies the same flag to the nested build.
    }
}
