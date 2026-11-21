// SPDX-FileCopyrightText: 2025-2026 rezky_nightky (oxyzenQ)
// SPDX-License-Identifier: AGPL-3.0-only OR Commercial

//! The generic BPF ringbuf mmap-drain lane (night-frontier-Z5).
//!
//! One implementation of the kernel's ringbuf mmap law, shared by the
//! two lanes that drain a ring as the consumer: the notice-drive lane
//! (PROOF 8, the pinned notices ring) and the tap-golden corpus mode
//! (the fresh events ring). The kernel contract lives here once:
//!
//! - vm_pgoff 0 is the CONSUMER page — the ONE writable mapping (a
//!   writable mapping anywhere else answers EPERM);
//! - vm_pgoff 1 is the producer page, vm_pgoff 2.. the data pages,
//!   all read-only;
//! - the data area spans ceil(data_size / page) pages plus the wrap
//!   page, data_size coming from the KERNEL (BPF_OBJ_GET_INFO_BY_FD),
//!   never from a local constant;
//! - each record carries an 8-byte header (`struct bpf_ringbuf_hdr`):
//!   the LOW u32 is the length (busy bit 14, discard bit 15), the
//!   high u32 is the record's own page offset;
//! - positions ride the acquire/release discipline libbpf's consumer
//!   uses; the drain publishes the advanced consumer position.

use std::io;
use std::sync::atomic::{AtomicU32, Ordering};

/// One committed record's raw bytes (header stripped, busy/discard
/// filtered by the caller-visible contract below).
pub(crate) struct RawRecord {
    pub(crate) bytes: Vec<u8>,
}

/// mmap-drain the ring as the consumer and return every committed
/// record whose committed length is at least `min_record_size`. The
/// drain ADVANCES the consumer position (the contract every ringbuf
/// consumer obeys), so a second drain observes only newer records.
pub(crate) fn drain_ring_records(
    map_fd: i32,
    data_size: usize,
    min_record_size: usize,
) -> io::Result<Vec<RawRecord>> {
    const PAGE_DEFAULT: usize = 4096;
    // SAFETY: sysconf is a plain libc query with no pointer arguments
    // and no side effects.
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as usize;
    let page_size = if page_size > 0 { page_size } else { PAGE_DEFAULT };
    // The kernel maps pages, not byte counts: the data area spans
    // ceil(data_size / page) pages plus the wrap page.
    let data_pages = data_size.div_ceil(page_size) + 1;
    // SAFETY: the consumer-page mapping rides the kernel's ringbuf
    // mmap law (writable only at pgoff 0); the length is the one page
    // the law grants.
    let cons = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            page_size,
            libc::PROT_READ | libc::PROT_WRITE,
            libc::MAP_SHARED,
            map_fd,
            0,
        )
    };
    if cons == libc::MAP_FAILED {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the producer-page mapping is PROT_READ at pgoff 1 —
    // read-only mappings are legal at any offset of the ring's own
    // mapping.
    let prod = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            page_size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            map_fd,
            page_size as libc::off_t,
        )
    };
    if prod == libc::MAP_FAILED {
        // SAFETY: the producer mapping failed, so cons has no other
        // owner — the failure path unmaps it exactly once.
        unsafe { libc::munmap(cons, page_size) };
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the data-area mapping is PROT_READ at pgoff 2.. — the
    // ringbuf mmap law refuses VM_WRITE here and grants read-only at
    // any offset.
    let data = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            data_pages * page_size,
            libc::PROT_READ,
            libc::MAP_SHARED,
            map_fd,
            2 * page_size as libc::off_t,
        )
    };
    if data == libc::MAP_FAILED {
        // SAFETY: the data mapping failed; cons has no other owner —
        // unmapped exactly once on this path.
        unsafe { libc::munmap(cons, page_size) };
        // SAFETY: prod has no other owner (data failed before any
        // consumer was built) — unmapped exactly once on this path.
        unsafe { libc::munmap(prod, page_size) };
        return Err(io::Error::last_os_error());
    }

    // SAFETY: the pages are MAP_SHARED mappings this function owns,
    // the two position words are 4-byte aligned kernel-stored counters
    // (the ringbuf contract), and the atomics only read them (the
    // consumer page's atomic additionally writes the drain position,
    // the one writable word the law grants).
    let prod_pos = unsafe { &*(prod as *const AtomicU32) };
    // SAFETY: the consumer page is the writable mapping this function
    // owns; the position word is the kernel's aligned u64 counter read
    // through its low 32 bits (the consumer publishes its drain). The
    // atomic writes follow libbpf's own release discipline.
    let cons_pos = unsafe { &*(cons as *const AtomicU32) };
    // SAFETY: from_raw_parts builds a read-only view over the data
    // mapping this function owns; the length is the mapped length
    // exactly, and nothing writes through it.
    let data_bytes =
        unsafe { std::slice::from_raw_parts(data as *const u8, data_pages * page_size) };

    let mut records = Vec::new();
    let start = prod_pos.load(Ordering::Acquire);
    let mut pos = cons_pos.load(Ordering::Relaxed);
    while pos != start {
        let off = (pos as usize) & (data_size - 1);
        let hdr = u64::from_le_bytes(
            data_bytes[off..off + 8]
                .try_into()
                .expect("off is 8-aligned inside the ring"),
        );
        const BUSY: u32 = 1 << 14;
        const DISCARD: u32 = 1 << 15;
        let len = ((hdr as u32) & !(BUSY | DISCARD)) as usize;
        let committed = (hdr as u32) & BUSY == 0;
        if committed && len >= min_record_size && off + 8 + len <= data_bytes.len() {
            records.push(RawRecord {
                bytes: data_bytes[off + 8..off + 8 + len].to_vec(),
            });
        }
        pos = pos.wrapping_add((8 + len).max(8).next_multiple_of(8) as u32);
    }
    // The drain: publish the advanced consumer position.
    cons_pos.store(pos, Ordering::Release);

    // SAFETY: each mapping is unmapped exactly once on the success
    // path this code closes — data, then cons, then prod, all owned
    // by this function.
    unsafe { libc::munmap(data, data_pages * page_size) };
    // SAFETY: cons was mapped by this function and unmapped exactly
    // once here (the data unmap above does not touch it).
    unsafe { libc::munmap(cons, page_size) };
    // SAFETY: prod was mapped by this function and unmapped exactly
    // once here — the last teardown on this path.
    unsafe { libc::munmap(prod, page_size) };
    Ok(records)
}

/// The ring's data size from the kernel (BPF_OBJ_GET_INFO_BY_FD —
/// bpf_map_info.max_entries at offset 16).
pub(crate) fn ringbuf_data_size(map_fd: i32) -> io::Result<usize> {
    const BPF_OBJ_GET_INFO_BY_FD: u64 = 15;
    let mut info = [0u8; 128];
    let mut attr = crate::progrun::Attr::new();
    attr.put32(0, map_fd as u32);
    attr.put32(4, info.len() as u32);
    attr.put64(8, info.as_mut_ptr() as u64);
    crate::progrun::bpf(BPF_OBJ_GET_INFO_BY_FD, &attr)
        .map_err(|e| io::Error::from_raw_os_error(e))?;
    let max_entries = u32::from_le_bytes(
        info[16..20]
            .try_into()
            .expect("the info buffer is fixed 128 bytes"),
    );
    if max_entries == 0 || !max_entries.is_power_of_two() {
        return Err(io::Error::other(format!(
            "the kernel reported a non-ringbuf-shaped size: {max_entries}"
        )));
    }
    Ok(max_entries as usize)
}
