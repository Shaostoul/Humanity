//! A one-writer, one-reader ring of video frames in shared memory.
//!
//! This is the "pipe" the spike measures: how a separate browser host process
//! would hand 1280x720 frames to the game without either process waiting on
//! the other. It is deliberately the simplest thing that can be correct.
//!
//! Both `tools/cef-probe` and `tools/frame-sink` include THIS FILE directly
//! (`#[path = "../../frame_shm.rs"] mod frame_shm;`) so the two processes can
//! never disagree about the layout: there is one copy of it.
//!
//! ## How it works
//!
//! Windows lets two processes map the same block of memory by name. We create
//! one block big enough for a small number of frames ("slots") and write into
//! them in turn. The reader looks at whichever slot was written most recently.
//!
//! Nothing locks. Instead each slot carries a **sequence number** that the
//! writer bumps twice: once before it starts writing (making it odd, meaning
//! "being written right now") and once after it finishes (making it even
//! again, meaning "complete"). The reader reads the number, copies the pixels,
//! then reads the number again. If it is the same even number both times, no
//! write overlapped the copy and the frame is whole. If not, the reader
//! simply tries again or waits for the next frame. This is the standard
//! "seqlock" pattern, and it costs the writer two atomic stores per frame.
//!
//! With enough slots the reader is essentially never unlucky: the writer has
//! to lap it completely to corrupt a read.
//!
//! ## Timestamps
//!
//! Each slot records the Windows performance counter at the moment the writer
//! finished it. That counter is the same clock in every process on the
//! machine, so the reader can subtract and get a true cross-process latency.
//! Rust's own `Instant` cannot do this, because its zero point is private to
//! each process.

#![allow(dead_code)] // each of the two crates uses a different half of this

use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};

use windows_sys::Win32::Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Memory::{
    CreateFileMappingW, MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_ALL_ACCESS,
    PAGE_READWRITE,
};
use windows_sys::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

/// Bytes at the front of the block, before the first slot. Generous and
/// cache-line aligned so the header never shares a line with pixel data.
pub const HEADER_BYTES: usize = 256;
/// Bytes at the front of each slot, before that slot's pixels.
pub const SLOT_HEADER_BYTES: usize = 64;
/// "HSM1" as a little-endian u32; a sanity check that the reader opened the
/// block we think it did.
pub const MAGIC: u32 = 0x3148_534D;

/// Field offsets inside the block header, in bytes.
mod hdr {
    pub const MAGIC: usize = 0;
    pub const WIDTH: usize = 4;
    pub const HEIGHT: usize = 8;
    pub const SLOT_COUNT: usize = 12;
    pub const SLOT_BYTES: usize = 16;
    /// Performance-counter ticks per second, so the reader need not ask.
    pub const QPF: usize = 24;
    /// Total frames the writer has completed since it started.
    pub const PRODUCED: usize = 32;
}

/// Field offsets inside a slot header, in bytes.
mod slot {
    /// Odd while the writer is inside this slot, even when it is whole.
    pub const SEQ: usize = 0;
    /// Which frame this is, counting from 1.
    pub const FRAME_ID: usize = 8;
    /// Performance counter at the moment the writer finished the slot.
    pub const PUBLISHED_QPC: usize = 16;
}

/// Read the machine's shared monotonic counter.
pub fn qpc() -> u64 {
    let mut v: i64 = 0;
    unsafe { QueryPerformanceCounter(&mut v) };
    v as u64
}

/// Ticks per second of that counter.
pub fn qpf() -> u64 {
    let mut v: i64 = 0;
    unsafe { QueryPerformanceFrequency(&mut v) };
    v as u64
}

/// A mapped shared-memory ring. Dropping it unmaps and closes the handle;
/// the block itself lives until every process that mapped it has gone.
pub struct FrameRing {
    handle: HANDLE,
    base: *mut u8,
    total_bytes: usize,
    pub width: u32,
    pub height: u32,
    pub slot_count: u32,
    pub slot_bytes: usize,
    pub qpf: u64,
    /// Which slot the writer will use next. Writer side only.
    next_slot: u32,
}

// The ring is a plain block of memory with atomics where they matter; sending
// the handle between threads inside one process is safe.
unsafe impl Send for FrameRing {}

impl FrameRing {
    /// CREATE the block (the browser host does this). `name` becomes a
    /// session-local object name, so two users on one machine do not collide.
    pub fn create(name: &str, width: u32, height: u32, slot_count: u32) -> Result<Self, String> {
        let pixel_bytes = width as usize * height as usize * 4;
        let slot_bytes = SLOT_HEADER_BYTES + pixel_bytes;
        let total = HEADER_BYTES + slot_bytes * slot_count as usize;

        let wide = wide_name(name);
        let handle = unsafe {
            CreateFileMappingW(
                INVALID_HANDLE_VALUE, // backed by the page file, not a real file
                std::ptr::null(),
                PAGE_READWRITE,
                (total >> 32) as u32,
                (total & 0xFFFF_FFFF) as u32,
                wide.as_ptr(),
            )
        };
        if handle.is_null() {
            return Err(format!("CreateFileMappingW failed for {name}"));
        }
        let base = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, total) };
        if base.Value.is_null() {
            unsafe { CloseHandle(handle) };
            return Err(format!("MapViewOfFile failed for {name}"));
        }
        let base = base.Value as *mut u8;

        let ring = Self {
            handle,
            base,
            total_bytes: total,
            width,
            height,
            slot_count,
            slot_bytes,
            qpf: qpf(),
            next_slot: 0,
        };
        // Stamp the header so a reader can discover the geometry.
        ring.put_u32(hdr::MAGIC, MAGIC);
        ring.put_u32(hdr::WIDTH, width);
        ring.put_u32(hdr::HEIGHT, height);
        ring.put_u32(hdr::SLOT_COUNT, slot_count);
        ring.put_u32(hdr::SLOT_BYTES, slot_bytes as u32);
        ring.put_u64(hdr::QPF, ring.qpf);
        ring.put_u64(hdr::PRODUCED, 0);
        for i in 0..slot_count {
            ring.slot_atomic(i, slot::SEQ).store(0, Ordering::Relaxed);
        }
        Ok(ring)
    }

    /// OPEN a block someone else created (the game does this).
    pub fn open(name: &str) -> Result<Self, String> {
        let wide = wide_name(name);
        let handle = unsafe { OpenFileMappingW(FILE_MAP_ALL_ACCESS, 0, wide.as_ptr()) };
        if handle.is_null() {
            return Err(format!("no shared memory named {name} (is the host running?)"));
        }
        // Map the header first to learn how big the whole thing is.
        let peek = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, HEADER_BYTES) };
        if peek.Value.is_null() {
            unsafe { CloseHandle(handle) };
            return Err("MapViewOfFile (header) failed".into());
        }
        let p = peek.Value as *const u8;
        let rd32 = |off: usize| unsafe { std::ptr::read_unaligned(p.add(off) as *const u32) };
        let rd64 = |off: usize| unsafe { std::ptr::read_unaligned(p.add(off) as *const u64) };
        let magic = rd32(hdr::MAGIC);
        let width = rd32(hdr::WIDTH);
        let height = rd32(hdr::HEIGHT);
        let slot_count = rd32(hdr::SLOT_COUNT);
        let slot_bytes = rd32(hdr::SLOT_BYTES) as usize;
        let qpf_v = rd64(hdr::QPF);
        unsafe { UnmapViewOfFile(peek) };
        if magic != MAGIC {
            unsafe { CloseHandle(handle) };
            return Err(format!("bad magic 0x{magic:08x} in {name}"));
        }

        let total = HEADER_BYTES + slot_bytes * slot_count as usize;
        let base = unsafe { MapViewOfFile(handle, FILE_MAP_ALL_ACCESS, 0, 0, total) };
        if base.Value.is_null() {
            unsafe { CloseHandle(handle) };
            return Err("MapViewOfFile (full) failed".into());
        }
        Ok(Self {
            handle,
            base: base.Value as *mut u8,
            total_bytes: total,
            width,
            height,
            slot_count,
            slot_bytes,
            qpf: qpf_v,
            next_slot: 0,
        })
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// How many frames the writer has completed.
    pub fn produced(&self) -> u64 {
        self.header_atomic(hdr::PRODUCED).load(Ordering::Acquire)
    }

    /// WRITER: copy one frame into the next slot. `pixels` must be exactly
    /// width * height * 4 bytes. Returns the frame id.
    pub fn publish(&mut self, pixels: &[u8]) -> u64 {
        let expect = self.width as usize * self.height as usize * 4;
        debug_assert_eq!(pixels.len(), expect, "frame is the wrong size for this ring");
        let i = self.next_slot;
        self.next_slot = (self.next_slot + 1) % self.slot_count;

        let seq_cell = self.slot_atomic(i, slot::SEQ);
        let seq = seq_cell.load(Ordering::Relaxed);
        // Odd: "I am writing here, do not trust these bytes."
        seq_cell.store(seq.wrapping_add(1), Ordering::Release);

        unsafe {
            let dst = self.base.add(HEADER_BYTES + self.slot_bytes * i as usize + SLOT_HEADER_BYTES);
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), dst, expect.min(pixels.len()));
        }

        let id = self.produced() + 1;
        self.put_u64_at_slot(i, slot::FRAME_ID, id);
        self.put_u64_at_slot(i, slot::PUBLISHED_QPC, qpc());
        // Even again: "this slot is whole."
        seq_cell.store(seq.wrapping_add(2), Ordering::Release);
        self.header_atomic(hdr::PRODUCED).store(id, Ordering::Release);
        id
    }

    /// READER: copy out the newest complete frame if it is newer than
    /// `last_seen`. Returns (frame_id, publish timestamp in counter ticks) on
    /// success, or None when there is nothing new (or the one attempt raced).
    pub fn read_newest(&self, last_seen: u64, out: &mut [u8]) -> Option<(u64, u64)> {
        let id = self.produced();
        if id == 0 || id <= last_seen {
            return None;
        }
        // The writer wrote frame `id` into the slot it reached in sequence,
        // so slot index is simply (id - 1) modulo the ring length.
        let i = ((id - 1) % self.slot_count as u64) as u32;
        let seq_cell = self.slot_atomic(i, slot::SEQ);
        let before = seq_cell.load(Ordering::Acquire);
        if before % 2 != 0 {
            return None; // writer is inside this slot right now
        }
        let frame_id = self.read_u64_at_slot(i, slot::FRAME_ID);
        let stamp = self.read_u64_at_slot(i, slot::PUBLISHED_QPC);
        let n = (self.width as usize * self.height as usize * 4).min(out.len());
        unsafe {
            let src =
                self.base.add(HEADER_BYTES + self.slot_bytes * i as usize + SLOT_HEADER_BYTES)
                    as *const u8;
            std::ptr::copy_nonoverlapping(src, out.as_mut_ptr(), n);
        }
        let after = seq_cell.load(Ordering::Acquire);
        if after != before {
            return None; // the writer landed on us mid-copy; skip this one
        }
        Some((frame_id, stamp))
    }

    // ---- small helpers over the raw block ----

    fn header_atomic(&self, off: usize) -> &AtomicU64 {
        unsafe { &*(self.base.add(off) as *const AtomicU64) }
    }
    fn slot_atomic(&self, i: u32, off: usize) -> &AtomicU64 {
        let at = HEADER_BYTES + self.slot_bytes * i as usize + off;
        unsafe { &*(self.base.add(at) as *const AtomicU64) }
    }
    fn put_u32(&self, off: usize, v: u32) {
        unsafe { std::ptr::write_unaligned(self.base.add(off) as *mut u32, v) };
    }
    fn put_u64(&self, off: usize, v: u64) {
        unsafe { std::ptr::write_unaligned(self.base.add(off) as *mut u64, v) };
    }
    fn put_u64_at_slot(&self, i: u32, off: usize, v: u64) {
        let at = HEADER_BYTES + self.slot_bytes * i as usize + off;
        unsafe { std::ptr::write_unaligned(self.base.add(at) as *mut u64, v) };
    }
    fn read_u64_at_slot(&self, i: u32, off: usize) -> u64 {
        let at = HEADER_BYTES + self.slot_bytes * i as usize + off;
        unsafe { std::ptr::read_unaligned(self.base.add(at) as *const u64) }
    }
}

impl Drop for FrameRing {
    fn drop(&mut self) {
        unsafe {
            let view = windows_sys::Win32::System::Memory::MEMORY_MAPPED_VIEW_ADDRESS {
                Value: self.base as *mut c_void,
            };
            UnmapViewOfFile(view);
            CloseHandle(self.handle);
        }
    }
}

/// Windows object names are UTF-16 and NUL terminated. "Local\" keeps the
/// name inside the current logon session.
fn wide_name(name: &str) -> Vec<u16> {
    let full = format!("Local\\{name}");
    full.encode_utf16().chain(std::iter::once(0)).collect()
}
