// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! Information subcommands: `doctor`, `list-presets`, `list-backends`,
//! and the no-subcommand `overview`.
//!
//! night-improve-22: the `testconf` subcommand died with `config.toml`
//! — there is no file left to validate. The compiled preset registry
//! is type-checked Rust data, proven in-range by the registry test
//! tree; a validator command for it would validate nothing.

use std::process;

use cpal::traits::{DeviceTrait, HostTrait};

#[cfg(feature = "ebpf")]
use crate::ebpf::pin;
use crate::session::preset;

/// Silence libasound's C-level stderr error log for the duration of
/// one device probe (night-improve-29, the CLI/UX tidy).
///
/// cpal's ALSA host resolves the `default` PCM by opening it; on a
/// host without a sound card libasound prints up to eight raw
/// `ALSA lib ...` diagnostic lines straight to stderr — C-library
/// noise the rendered verdict already answers honestly. The guard
/// swaps in a silent handler for the probe's lifetime and restores
/// the library default on drop, so the player path (where a raw ALSA
/// line is real diagnosis) stays loud. alsa-sys is cpal's own pin —
/// the target-gated dependency adds zero crates to the lock.
#[cfg(target_os = "linux")]
struct QuietAlsa;

#[cfg(target_os = "linux")]
impl QuietAlsa {
    fn new() -> Self {
        // The handler type carries a C-variadic tail; defining a
        // variadic `extern "C" fn` is a nightly-only surface, so the
        // no-op non-variadic shape is transmuted into the handler slot
        // (the established pattern: both are nullable function
        // pointers of identical layout, the callee reads no arguments,
        // and the SysV x86-64 ABI passes the variadic bookkeeping in a
        // call-clobbered register a non-variadic callee ignores).
        unsafe {
            alsa_sys::snd_lib_error_set_handler(std::mem::transmute::<
                unsafe extern "C" fn(
                    *const std::os::raw::c_char,
                    std::os::raw::c_int,
                    *const std::os::raw::c_char,
                    std::os::raw::c_int,
                    *const std::os::raw::c_char,
                ),
                alsa_sys::snd_lib_error_handler_t,
            >(
                silent_alsa_handler
                    as unsafe extern "C" fn(
                        *const std::os::raw::c_char,
                        std::os::raw::c_int,
                        *const std::os::raw::c_char,
                        std::os::raw::c_int,
                        *const std::os::raw::c_char,
                    ),
            ));
        }
        QuietAlsa
    }
}

#[cfg(target_os = "linux")]
impl Drop for QuietAlsa {
    fn drop(&mut self) {
        unsafe { alsa_sys::snd_lib_error_set_handler(None) };
    }
}

/// The silent handler: libasound's documented contract accepts any
/// no-op with this signature (the variadic tail is transmuted away
/// in `QuietAlsa::new` — see the note there); the rendered verdict
/// lines are the user-facing truth instead.
#[cfg(target_os = "linux")]
unsafe extern "C" fn silent_alsa_handler(
    _file: *const std::os::raw::c_char,
    _line: std::os::raw::c_int,
    _function: *const std::os::raw::c_char,
    _err: std::os::raw::c_int,
    _fmt: *const std::os::raw::c_char,
) {
}

/// Print a system-health diagnostic report (build facts, kernel/eBPF
/// verdict, audio).
///
/// The report leads with the BUILD facts block (night-improve-13, the
/// zelynic night-improve-66 doctor completeness): version, commit,
/// variant, build timestamp, rustc, profile — the binary answers "which
/// exact artifact am I?" before anything about the host, because on a
/// foreign build every host verdict below reads differently. Every fact
/// is compile-time truth assembled by [`crate::info::build`] (the
/// Hinnant clock stamps the build time, no time crate).
pub fn cmd_doctor() {
    println!("{}\n", crate::output::section("=== zylaxion doctor ==="));

    // Privilege awareness (night-improve-16, the audit's warn lane):
    // doctor stays reachable as root (a diagnostic must diagnose), but
    // every verdict below would describe ROOT's session — root is
    // always "in" the input group, root's audio server is a different
    // instance, root's XDG_RUNTIME_DIR may not even be the user's.
    // Name the distortion up front so the report cannot mislead.
    if crate::session::privilege::euid_is_root() {
        crate::output::warning(
            "running as root — the verdicts below describe root's session, not yours",
        );
        println!(
            "      {}",
            crate::output::tip_line("tip: re-run without sudo")
        );
        println!();
    }

    let mut ok = true;

    // 0. Build facts (compile-time truth — cannot fail, never counts
    // against the ok verdict).
    let build = crate::info::build::BuildInfo::collect();
    println!("{}", crate::output::section("[1/4] Build facts..."));
    println!("      Version:    v{}", build.version);
    println!("      Commit:     {}", build.git_sha);
    println!("      Variant:    {}", build.variant);
    println!("      Built:      {}", build.build_time);
    println!("      Rustc:      {}", build.rustc_version);
    println!("      Profile:    {}", build.profile);

    // 1. Kernel / eBPF capability (night-improve-23, the eBPF-only
    //    floor): the input tap is a passive kernel-side probe on the
    //    exported input_event() function and the state store is pinned
    //    maps on bpffs — the kernel IS the
    //    platform now. The owner's minimum is 5.15; below it the
    //    verdict is "verified unsupported" and nothing else in this
    //    report can save the run. One probe feeds both this report
    //    and the attach path, so the two surfaces can never drift.
    let kernel = crate::session::kernel::probe();
    println!(
        "\n{}",
        crate::output::section("[2/4] Kernel / eBPF capability...")
    );
    if kernel.release.is_empty() {
        println!(
            "      {}",
            crate::output::fail_line(
                "FAIL: kernel release unreadable (/proc/sys/kernel/osrelease)"
            )
        );
        ok = false;
    } else {
        println!("      Kernel:   {}", kernel.release);
        if kernel.supported() {
            println!(
                "      {}",
                crate::output::ok_line(&format!(
                    "PASS: eBPF: {} ({} meets the 5.15 minimum)",
                    kernel.verdict(),
                    kernel.release
                ))
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(&format!(
                    "FAIL: eBPF: {} ({} is below the 5.15 minimum — zylaxion is eBPF-only, upgrade the kernel)",
                    kernel.verdict(),
                    kernel.release
                ))
            );
            ok = false;
        }
        // BTF: the fentry lane's enabler (night-improve-25). The chain
        // does not NEED it — the kprobe steps run without BTF — so it
        // is reported for completeness, never fatal.
        println!(
            "      BTF:      {} ({})",
            if kernel.btf { "present" } else { "absent" },
            if kernel.btf {
                "enables the fentry lane"
            } else {
                "the chain falls back to the kprobe lanes"
            }
        );
        if kernel.bpffs {
            println!(
                "      {}",
                crate::output::ok_line("PASS: bpffs: mounted at /sys/fs/bpf")
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(
                    "FAIL: bpffs: not mounted at /sys/fs/bpf — the pinned state store has nowhere to live"
                )
            );
            println!(
                "      {}",
                crate::output::tip_line(
                    "fix: sudo mount -t bpf bpf /sys/fs/bpf (normally automatic)"
                )
            );
            ok = false;
        }
        // The input-tap verdict, rendered VERBATIM (night-improve-25):
        // the probe's preflight of the fentry → kprobe chain — one of
        // the four canonical strings, self-explaining, and the doctor
        // adds nothing to it (the attach attempt stays the
        // authoritative verdict).
        println!("      {}", kernel.input_tap);
        // The last attach failure, if one left its marker
        // (night-improve-25): a failed attach plants a small pinned map
        // carrying the classified error code; naming it here is the
        // doctor's honesty duty — the tap is not attached, and this
        // line says exactly which step of the chain failed and how it
        // was classified.
        #[cfg(feature = "ebpf")]
        if let Some(err) = crate::ebpf::loader::last_attach_error() {
            println!(
                "      note: last attach failed: {err} — `sudo zylaxion attach` prints the \
                 per-step log"
            );
        }
        if kernel.lockdown_readable && kernel.lockdown_blocks_bpf {
            println!(
                "      {}",
                crate::output::fail_line(
                    "FAIL: lockdown: 'confidentiality' active — the kernel withdraws BPF"
                )
            );
            println!(
                "      {}",
                crate::output::tip_line(
                    "fix: choose a kernel lockdown mode that allows BPF (integrity or none)"
                )
            );
            ok = false;
        }
        // The unprivileged-bpf posture (night-badas-4 round 5): the
        // control-plane verbs run as a plain user only when the kernel
        // allows unprivileged bpf(). A distro that builds the gate in
        // (Ubuntu does, CONFIG_BPF_UNPRIV_DEFAULT_OFF=y) locks the
        // verbs out for the whole boot — the doctor's duty is to name
        // it, the same way lockdown is named.
        if kernel.unpriv_bpf_allowed() {
            println!(
                "      {}",
                crate::output::ok_line(
                    "PASS: unprivileged bpf: allowed — the verbs run as a plain user"
                )
            );
        } else {
            println!(
                "      {}",
                crate::output::fail_line(&format!("FAIL: {}", kernel.unpriv_bpf_verdict()))
            );
            ok = false;
        }
    }

    // 3. XDG_RUNTIME_DIR check.
    println!("\n{}", crate::output::section("[3/4] XDG_RUNTIME_DIR..."));
    match std::env::var("XDG_RUNTIME_DIR") {
        Ok(dir) => {
            let path = std::path::Path::new(&dir);
            if path.is_dir() {
                println!(
                    "      {}",
                    crate::output::ok_line(&format!("PASS: $XDG_RUNTIME_DIR = {dir}"))
                );
            } else {
                println!(
                    "      {}",
                    crate::output::fail_line(&format!(
                        "FAIL: $XDG_RUNTIME_DIR = {dir} (not a directory)"
                    ))
                );
                ok = false;
            }
        }
        Err(_) => {
            println!(
                "      {}",
                crate::output::fail_line("FAIL: $XDG_RUNTIME_DIR is not set")
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: export XDG_RUNTIME_DIR=/run/user/$(id -u)")
            );
            ok = false;
        }
    }

    // 4. Audio server check (night-improve-29: the probe now OPENS the
    //    device — a name without a working config was a false PASS on
    //    hosts whose `default` PCM cannot open — and the probe rides
    //    the quiet-ALSA guard so libasound's C-level error log cannot
    //    spew across the report. The compact render matches the
    //    overview's audio line, one shape family-wide).
    println!("\n{}", crate::output::section("[4/4] Audio server..."));
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    match cpal::default_host().default_output_device().map(|d| {
        let name = d.name().unwrap_or_else(|_| "<unknown>".into());
        (name, d.default_output_config())
    }) {
        Some((name, Ok(config))) => {
            println!(
                "      {}",
                crate::output::ok_line(&format!(
                    "PASS: default output device: {name} ({} Hz, {}ch, {:?})",
                    config.sample_rate().0,
                    config.channels(),
                    config.sample_format()
                ))
            );
        }
        Some((name, Err(e))) => {
            println!(
                "      {}",
                crate::output::fail_line(&format!(
                    "FAIL: default output device ({name}) present but not openable: {e}"
                ))
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: start PipeWire or PulseAudio")
            );
            ok = false;
        }
        None => {
            println!(
                "      {}",
                crate::output::fail_line("FAIL: no audio output device found")
            );
            println!(
                "      {}",
                crate::output::tip_line("fix: start PipeWire or PulseAudio")
            );
            ok = false;
        }
    }

    println!();
    if ok {
        println!(
            "{}",
            crate::output::ok_line("All checks passed — zylaxion is ready to run.")
        );
    } else {
        println!(
            "{}",
            crate::output::fail_line("Some checks failed — see above for fixes.")
        );
        process::exit(1);
    }
}

/// List every acoustic preset in the compiled registry and mark the
/// one the session context would activate right now (the
/// `ZYLAXION_PRESET` selection when it names a registered preset,
/// the compiled default otherwise).
///
/// Exits 0 always — the registry is compile-time data; there is no
/// file to fail finding and no parse to fail.
pub fn cmd_list_presets() {
    let active = preset::context_preset_name();
    let env_name = std::env::var(preset::PRESET_ENV_VAR)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty());
    println!("Available presets (compiled into this binary):\n");
    print!("{}", presets_table_from(active, env_name.as_deref()));
}

/// The pure render behind `list-presets` (night-improve-29): ONE line
/// per preset — the name in a fixed-width column, the description
/// beside it, the session-active row marked at the end — and no
/// how-to footer (the help and the overview teach switching; a list
/// lists). The env honesty note names an override that names nothing.
fn presets_table_from(active: &str, env_preset: Option<&str>) -> String {
    let registry = crate::profiles::registry_all();
    let width = registry.iter().map(|e| e.name.len()).max().unwrap_or(0);
    let mut out = String::new();
    for entry in registry.iter() {
        out.push_str("  ");
        out.push_str(&format!("{:<width$}", entry.name));
        out.push_str("  ");
        out.push_str(entry.description);
        if entry.name == active {
            out.push_str(&crate::output::ok_marker("  <- active"));
        }
        out.push('\n');
    }
    // The honesty note: an env selection that names nothing renders
    // as if it were absent (the compiled default is marked) — the
    // note names the truth so the marker cannot mislead.
    if let Some(env_name) = env_preset
        && !registry.iter().any(|e| e.name == env_name)
    {
        out.push_str(&crate::output::tip_line(&format!(
            "\nnote: {}=\"{env_name}\" names no registered preset — using {active}",
            preset::PRESET_ENV_VAR
        )));
        out.push('\n');
    }
    out
}

/// List the audio surface via cpal (night-improve-29 render): the
/// host, then the default output in the SAME compact shape the
/// overview and the doctor carry — one device, one line. The probe
/// rides the quiet-ALSA guard; on a host with no sound card the
/// honest verdict line replaces libasound's raw stderr spew.
pub fn cmd_list_backends() {
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    let host = cpal::default_host();
    println!("Audio host: {}", host.id().name());
    match host.default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            match device.default_output_config() {
                Ok(config) => println!(
                    "Default output: {name} ({} Hz, {}ch, {:?})",
                    config.sample_rate().0,
                    config.channels(),
                    config.sample_format()
                ),
                Err(e) => {
                    println!("Default output: {name} (present but not openable: {e})")
                }
            }
        }
        None => println!("Default output: none found"),
    }
}

/// Quick status overview shown when `zylaxion` is run with no
/// subcommand (v10.2.0+ — user feedback). night-improve-23 rewrite:
/// the daemon lines are gone; the tap residency, the player state, and
/// the active posture replace them — the same facts `status` prints,
/// in the compact no-subcommand shape.
pub fn cmd_overview() {
    println!("=== zylaxion ===\n");

    // 1. Kernel verdict (both lanes — the kernel probe has no deps).
    let kernel = crate::session::kernel::probe();
    println!("  kernel:  {} ({})", kernel.release, kernel.verdict());

    // 2. Tap residency + active posture (feature lane only; the dormant
    //    binary says so below).
    #[cfg(feature = "ebpf")]
    {
        if !pin::any_pin_present() {
            println!("  tap:     not attached (sudo zylaxion attach)");
        } else if pin::all_pins_present() {
            // The chain step that owns the tap (night-improve-25) —
            // the same TAP_KIND read the status line carries.
            let step = crate::commands::sound::open_control()
                .ok()
                .and_then(|c| c.get(&5u32, 0).ok())
                .and_then(crate::ebpf::loader::TapKind::from_slot)
                .map(|k| k.label().to_string())
                .unwrap_or_else(|| "unknown step".to_string());
            println!("  tap:     attached ({step})");
        } else {
            println!("  tap:     PARTIAL pin set (sudo zylaxion attach replants it)");
        }
        if let Ok(control) = crate::commands::sound::open_control() {
            let enabled = control.get(&0u32, 0).unwrap_or(0) == 1;
            let preset_index = control.get(&1u32, 0).unwrap_or(0);
            let preset_name = crate::profiles::registry_all()
                .get(preset_index as usize)
                .map(|e| e.name)
                .unwrap_or("<unknown>");
            let volume = control.get(&2u32, 0).unwrap_or(100);
            let pid = control.get(&3u32, 0).unwrap_or(0);
            println!("  sound:   {}", if enabled { "on" } else { "off" });
            println!("  preset:  {preset_name}");
            println!("  volume:  {volume}% (100 = the compiled default)");
            match pid {
                0 => println!("  player:  not running"),
                p => println!("  player:  pid {p}"),
            }
        } else if pin::any_pin_present() {
            println!("  control: not openable by this user (pin ownership?)");
        }
    }
    #[cfg(not(feature = "ebpf"))]
    println!("  ebpf:    dormant build (--no-default-features) — no tap surface");

    // 3. Audio device (the quiet-ALSA guard keeps libasound's C-level
    //    error log off the overview on cardless hosts).
    #[cfg(target_os = "linux")]
    let _quiet = QuietAlsa::new();
    let host = cpal::default_host();
    match host.default_output_device() {
        Some(device) => {
            let name = device.name().unwrap_or_else(|_| "<unknown>".into());
            if let Ok(dev_config) = device.default_output_config() {
                println!(
                    "  audio:   {name} ({} Hz, {}ch, {:?})",
                    dev_config.sample_rate().0,
                    dev_config.channels(),
                    dev_config.sample_format()
                );
            } else {
                println!("  audio:   {name} (config unavailable)");
            }
        }
        None => {
            println!("  audio:   no device found");
        }
    }

    println!();
    println!("  zylaxion on                      # start the sound");
    println!("  zylaxion preset --switch <name>  # switch sound live");
    println!("  zylaxion status                  # the full truth");
    println!();
}

#[cfg(test)]
#[path = "../../test/commands/info_tests.rs"]
mod tests;
