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
        // no-op non-variadic shape is transmuted into the handler slot.
        // The invariant is arch-general across every Linux target this
        // crate compiles for (night-dragon-lts: the old comment argued
        // only SysV x86-64; the guard builds on aarch64 and arm too):
        //
        // 1. The transmute itself is layout-safe — a function pointer
        //    and `Option<function pointer>` share the same
        //    representation (the niche optimization), so the pointer
        //    identity survives the type change byte-for-byte.
        // 2. The call is safe because the callee reads NO argument:
        //    the unnamed variadic tail differs per ABI — SysV x86-64
        //    passes its bookkeeping in `al` (caller-side, set by
        //    libasound, read by no one here); AAPCS64 passes every
        //    argument in x0-x7 then the stack; ARM EABI pushes the
        //    whole variadic call on the stack — and none of that can
        //    affect an empty function that never loads a parameter.
        //    No callee-side prologue (va_start register saving) is
        //    assumed, because our body has none.
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
