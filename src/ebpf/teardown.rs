// Copyright (C) 2026 rezky_nightky
// SPDX-License-Identifier: GPL-3.0-only

//! The loader's teardown side (night-improve-26): unpinning everything
//! and reading the failure evidence back. Split from loader.rs by the
//! 600-LOC no-mercy cap — one theme per file, the zelynic discipline.
//!
//! Two surfaces:
//!
//! - [`detach`] — unpin everything (idempotent, best-effort). After a
//!   successful detach NOTHING of zylaxion exists in the kernel: every
//!   pin unlinked, the failure marker gone, and the pin directory
//!   itself rmdir'd through the classified night-improve-26 cleanup
//!   (foreign entries are left and named, unexpected errors surface).
//! - [`last_attach_error`] — the doctor's read side of the marker lane:
//!   the classified verdict a failed attach planted on bpffs.

use std::path::Path;

use super::attach_error::AttachError;
use super::pin;

/// Detach the tap: unpin everything (idempotent, best-effort). Returns
/// a human-readable summary. After a successful detach NOTHING of
/// zylaxion exists in the kernel.
pub fn detach() -> Result<String, AttachError> {
    if !pin::any_pin_present() {
        return Ok("tap is not attached — nothing to detach".to_string());
    }
    let (removed, dir_outcome) = pin::clear_pins();
    let missing: Vec<_> = pin::ALL_PINS
        .iter()
        .chain(std::iter::once(&pin::PIN_MAP_ERROR))
        .filter(|p| Path::new(p).exists())
        .collect();
    if !missing.is_empty() {
        return Err(AttachError::Unknown {
            detail: format!(
                "tap detach incomplete: {removed:?} removed, but {} could not be removed \
                 (owned by another user? remove with sudo)",
                missing
                    .iter()
                    .map(|p| p.as_ref())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ),
        });
    }
    // The classified rmdir (night-improve-26): the two benign outcomes
    // read as a clean detach; foreign entries are handled-not-fatal and
    // reported honestly; anything unexpected is surfaced, not swallowed.
    match dir_outcome {
        pin::PinDirRemoval::Removed | pin::PinDirRemoval::AlreadyAbsent => Ok(format!(
            "tap detached: {} pin(s) removed — nothing of zylaxion remains in the kernel",
            removed.len()
        )),
        pin::PinDirRemoval::LeftForeignEntries => Ok(format!(
            "tap detached: {} pin(s) removed — the pin directory holds entries zylaxion \
             did not create and was left in place (foreign entries are not ours to remove)",
            removed.len()
        )),
        pin::PinDirRemoval::Unexpected(e) => Err(AttachError::Unknown {
            detail: format!(
                "tap detached: {} pin(s) removed, but the pin directory could not be \
                 removed: {e}",
                removed.len()
            ),
        }),
    }
}

/// The last attach failure recorded in the marker map, if one is
/// planted and readable — the doctor's read side of the marker lane.
pub fn last_attach_error() -> Option<AttachError> {
    use aya::maps::MapData;
    let data = MapData::from_pin(pin::PIN_MAP_ERROR).ok()?;
    let owned = aya::maps::Map::Array(data);
    let marker: aya::maps::Array<MapData, u32> = aya::maps::Array::try_from(owned).ok()?;
    let code = marker.get(&0u32, 0).ok()?;
    AttachError::from_code(code)
}
