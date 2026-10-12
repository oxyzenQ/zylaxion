// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Build script — stamps the compile-time facts the binary reports about
//! itself:
//!
//! - `GIT_HASH` — the short commit hash the crate was built from
//!   (consumed by the version report's `Build:` line).
//! - `ZYLAXION_BUILD_TIME` — the UTC build timestamp, computed by Howard
//!   Hinnant's `civil_from_days` algorithm (night-improve-13, the zelynic
//!   NIGHT-hunt-6 lineage) so the version report's `Build-time:` line and
//!   the doctor's BUILD block never need a time crate (chrono/jiff and
//!   friends stay out of the dependency tree entirely).
//! - `ZYLAXION_RUSTC_VERSION` / `ZYLAXION_PROFILE` / `ZYLAXION_OPT_LEVEL`
//!   — the toolchain and profile facts for the doctor's BUILD block (the
//!   zelynic night-improve-66 completeness).
//! - `ZYLAXION_BUILD` — the canonical build label forwarded from the
//!   environment when an alias/CI build stamps one (see `build_info`).
//! - `ZYLAXION_EBPF_LANE` — which lane produced (or skipped) the embedded
//!   eBPF object (night-improve-23): `source-built` (a git checkout with
//!   the ebpf/ tree — the nested nightly cross-build), `registry-prebuilt`
//!   (a registry tarball — the maintainer-built object staged from
//!   ebpf-prebuilt/), or the dormant refusal (the `ebpf` feature off).
//!
//! Every fallback degrades gracefully — a missing fact renders as
//! "unknown", never a failed build.

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

/// The one production BPF object (night-improve-23): the tap.
const TAP_OBJECT: &str = "zylaxion-tap";

fn main() {
    // Re-run this script whenever HEAD (or the work tree state) changes so
    // the embedded hash stays in sync with what was actually compiled.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");

    let hash = match git_short_hash() {
        Some(h) => h,
        None => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", hash);

    // The UTC build timestamp (Hinnant civil-from-days, std clock only —
    // no time crate). Consumed by build_info::build_time().
    println!(
        "cargo:rustc-env=ZYLAXION_BUILD_TIME={}",
        format_build_time_utc()
    );

    // The doctor's BUILD facts (zelynic night-improve-66 lineage): the
    // rustc cargo itself drove, the profile mode, and the real
    // optimization level — so a RUSTFLAGS override shows up honestly.
    let rustc_version = rustc_stamp();
    println!("cargo:rustc-env=ZYLAXION_RUSTC_VERSION={rustc_version}");
    println!(
        "cargo:rustc-env=ZYLAXION_PROFILE={}",
        std::env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string())
    );
    println!(
        "cargo:rustc-env=ZYLAXION_OPT_LEVEL={}",
        std::env::var("OPT_LEVEL").unwrap_or_else(|_| "?".to_string())
    );

    // Forward the ZYLAXION_BUILD canonical label into the compile-time env
    // consumed by build_info::build_label(). The label is exported by CI
    // / release scripts (the release-matrix platform id, e.g.
    // `linux-amd64-v3-gnu`); plain `cargo build` leaves it unset and the
    // compile-time arch+libc detection answers instead.
    println!("cargo:rerun-if-env-changed=ZYLAXION_BUILD");
    if let Ok(label) = std::env::var("ZYLAXION_BUILD") {
        println!("cargo:rustc-env=ZYLAXION_BUILD={label}");
    }

    // night-improve-23: stage the eBPF object into OUT_DIR for the
    // loader's include_bytes! embedding.
    build_ebpf_objects();
}

/// Runs `git rev-parse --short HEAD` from the workspace root and returns
/// the trimmed short hash on success.
fn git_short_hash() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8(output.stdout).ok()?;
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Ask the compiler cargo itself drives for its version string
/// ("rustc 1.98.1 (48a229cea 2026-09-01)" shape). "unknown" only when the
/// compiler could not answer `--version` — which could not produce a
/// binary either, so the degraded lane is unreachable in practice.
fn rustc_stamp() -> String {
    let rustc = match std::env::var("RUSTC") {
        Ok(r) => r,
        Err(_) => return "unknown".to_string(),
    };
    let output = match Command::new(rustc).arg("--version").output() {
        Ok(o) => o,
        Err(_) => return "unknown".to_string(),
    };
    if !output.status.success() {
        return "unknown".to_string();
    }
    String::from_utf8(output.stdout)
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string())
}

/// Read the system clock and render the build time as
/// `M/D/YYYY HH:MM (UTC)`. "unknown" only when the host clock sits
/// before the UNIX epoch (unreachable in practice).
fn format_build_time_utc() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let Ok(secs) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return "unknown".to_string();
    };
    let total_secs: i64 = i64::try_from(secs.as_secs()).unwrap_or(0);
    format_unix_secs_as_build_time(total_secs)
}

/// Pure formatting function — takes unix-epoch seconds and returns
/// `M/D/YYYY HH:MM (UTC)`. Separated from `format_build_time_utc` so the
/// algorithm is unit-testable without depending on the wall clock.
///
/// Algorithm: split `total_secs` into days + seconds-of-day, then use
/// Howard Hinnant's `civil_from_days` algorithm
/// (http://howardhinnant.github.io/date_algorithms.html) to convert
/// days-since-epoch to (year, month, day). All arithmetic is on `i64` to
/// avoid unsigned-underflow issues when subtracting the 719468-day shift
/// constant. The runtime twin of this core lives in
/// `src/info/clock.rs` (the daemon log timestamp lane) — the pairing is
/// deliberate: zero time crates on both surfaces.
fn format_unix_secs_as_build_time(total_secs: i64) -> String {
    let days_since_epoch = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let hour = secs_of_day / 3_600;
    let minute = (secs_of_day % 3_600) / 60;

    let (year, month, day) = civil_from_days(days_since_epoch);

    format!("{month}/{day}/{year} {hour:02}:{minute:02} (UTC)")
}

/// Howard Hinnant's `civil_from_days`: converts days-since-1970-01-01
/// to (year, month, day) in the proleptic Gregorian calendar.
/// http://howardhinnant.github.io/date_algorithms.html#civil_from_days
fn civil_from_days(days_since_epoch: i64) -> (i64, u32, u32) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

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
//                       ebpf-prebuilt/ is staged verbatim, provenance in
//                       ebpf-prebuilt/manifest.toml. This is the lane
//                       that lets a plain `cargo install zylaxion` land
//                       the full-featured binary on a stable toolchain.
//   source-built      — a git checkout: the ebpf/ crate is cross-compiled
//                       by a nested cargo invocation under the dated
//                       nightly pin, and the artifact is staged after
//                       structural validation (with one self-heal retry).
//
// The source-built lane keeps the zelynic hardening that earned its
// place: the prerequisite preflight (fail before compile time with the
// one-command fix), the RUSTFLAGS poison strip plus the v3 force (env
// rustflags shadow config; host-CPU flags are poison for bpfel), and
// the structural ELF validation with the damaged-artifact self-heal
// (cargo's freshness is fingerprint-plus-existence, never output
// integrity — a truncated object would ride every later build).

fn build_ebpf_objects() {
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

    // Registry lane: no ebpf/ tree rides a crates.io extract.
    if !ebpf_dir.join("Cargo.toml").is_file() {
        stage_registry_prebuilt_objects(&manifest_dir);
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

    println!("cargo:rustc-env=ZYLAXION_EBPF_LANE=source-built");
}

fn stage_object(out_dir: &std::path::Path, data: &[u8]) {
    let dst = out_dir.join(TAP_OBJECT);
    std::fs::write(&dst, data)
        .unwrap_or_else(|e| panic!("failed to stage {} into OUT_DIR: {e}", dst.display()));
}

/// Registry lane: stage the maintainer-built object from ebpf-prebuilt/.
/// A missing or damaged prebuilt object is an INCOMPLETE crate — panic
/// with the honest diagnosis and the two working remedies.
fn stage_registry_prebuilt_objects(manifest_dir: &std::path::Path) {
    let prebuilt_dir = manifest_dir.join("ebpf-prebuilt");
    // Watched only on the lane that consumes it: a git checkout builds
    // its object from ebpf/ and never reads this directory.
    println!("cargo:rerun-if-changed={}", prebuilt_dir.display());
    let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let data =
        read_validated_ebpf_object(&prebuilt_dir.join(TAP_OBJECT)).unwrap_or_else(|problem| {
            panic!(
                "the ebpf feature was requested from a registry tarball (no ebpf/ \
             workspace rides a registry package), and the prebuilt object \
             {TAP_OBJECT} is missing or damaged ({problem}). This crate is INCOMPLETE: \
             the ebpf-prebuilt/ lane that makes `cargo install zylaxion` full-featured \
             did not survive packaging. Meanwhile build from a git checkout \
             (git clone https://github.com/oxyzenQ/zylaxion, then cargo build --release) \
             or install the binary from GitHub Releases"
            )
        });
    stage_object(&out_dir, &data);
    println!("cargo:rustc-env=ZYLAXION_EBPF_LANE=registry-prebuilt");
}

/// Verify the two source-build prerequisites BEFORE the nested build
/// spends compile time: the dated nightly pin (listed in rustup) and the
/// pinned bpf-linker release on PATH. Each failure names its fix.
fn preflight_ebpf_prerequisites(toolchain: &str) {
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
fn run_nested_ebpf_build(ebpf_dir: &std::path::Path) {
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
fn delete_damaged_artifact(release_dir: &std::path::Path, name: &str) {
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
fn read_validated_ebpf_object(path: &std::path::Path) -> Result<Vec<u8>, String> {
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
fn validate_ebpf_object(data: &[u8]) -> Result<(), String> {
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
fn strip_host_poison_rustflags(cmd: &mut Command) {
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
fn strip_host_poison(value: &str, sep: char) -> Option<String> {
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
fn force_bpf_v3_rustflags(cmd: &mut Command) {
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

// The build script's own test suite is NOT executed by `cargo test`
// (cargo never runs build-script tests) — it is a standing contract that
// is verified standalone via:
//   rustc --edition 2024 --test crates/zylaxion/build.rs && ./build
// Every constant below was verified against `date -u -d @<epoch>` at
// authoring time (not from memory) — the same discipline the zelynic
// reference build.rs records.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_time_format_matches_known_unix_epochs() {
        // UNIX epoch: 1970-01-01 00:00:00 UTC.
        assert_eq!(format_unix_secs_as_build_time(0), "1/1/1970 00:00 (UTC)");

        // 2000-01-01 00:00:00 UTC = 946_684_800 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(946_684_800),
            "1/1/2000 00:00 (UTC)"
        );

        // 2024-02-29 12:34:00 UTC = 1_709_210_040 seconds since epoch.
        // Leap-day boundary check — Feb 29 must not roll to Mar 1.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_040),
            "2/29/2024 12:34 (UTC)"
        );

        // 2026-08-04 15:30:00 UTC = 1_785_857_400 seconds since epoch.
        assert_eq!(
            format_unix_secs_as_build_time(1_785_857_400),
            "8/4/2026 15:30 (UTC)"
        );
    }

    #[test]
    fn build_time_format_truncates_sub_minute_seconds() {
        // 1_709_210_440 = 2024-02-29 12:40:40 UTC: the 40 sub-minute
        // seconds are dropped (minute precision), never rounded up.
        assert_eq!(
            format_unix_secs_as_build_time(1_709_210_440),
            "2/29/2024 12:40 (UTC)"
        );
    }

    #[test]
    fn build_time_format_handles_negative_seconds_gracefully() {
        // Pre-epoch timestamps (negative seconds) should still produce
        // a valid proleptic Gregorian date via the algorithm's signed
        // arithmetic, not panic or underflow. -60 = 1969-12-31 23:59 UTC.
        let result = format_unix_secs_as_build_time(-60);
        assert!(
            result.ends_with("(UTC)"),
            "negative-epoch result should still be (UTC)-suffixed: {result}"
        );
        assert!(
            result.contains("1969"),
            "negative-epoch result should land in 1969: {result}"
        );
    }
}
