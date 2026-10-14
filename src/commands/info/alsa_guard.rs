// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The ALSA stderr guard (night-improve-29, the CLI/UX tidy): one
//! device probe's lifetime without libasound's C-level error log.

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
pub(super) struct QuietAlsa;

#[cfg(target_os = "linux")]
impl QuietAlsa {
    pub(super) fn new() -> Self {
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
