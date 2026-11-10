//! The engine's own memory, for its `status` and `loaded` answers (rung 2, 2026-11-09; the design's
//! 12.1, "memory against its cap").
//!
//! Two numbers, because a model file is read two ways at once:
//! - **committed** (Windows' "private bytes", `PagefileUsage`): memory the engine owns, the working
//!   memory of a conversation, the reordered weights llama.cpp makes for faster kernels, its buffers.
//!   The job object's memory cap counts exactly this, so it is what a load is measured against.
//! - **working set**: everything in physical memory now, the parts of the model file read straight
//!   from disk (llama.cpp maps the file) included. Task Manager's "Memory" column is close to this.
//!
//! Read from the operating system on each call, never kept. Windows only; other systems answer None.

/// The engine's memory now, in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usage {
    pub committed: u64,
    pub working_set: u64,
    pub peak_committed: u64,
}

impl Usage {
    pub fn committed_mb(&self) -> u64 {
        self.committed / (1024 * 1024)
    }
}

/// Read it now.
pub fn usage() -> Option<Usage> {
    #[cfg(windows)]
    {
        // PROCESS_MEMORY_COUNTERS (psapi.h), field for field.
        #[repr(C)]
        #[derive(Default)]
        struct Counters {
            cb: u32,
            page_fault_count: u32,
            peak_working_set_size: usize,
            working_set_size: usize,
            quota_peak_paged_pool_usage: usize,
            quota_paged_pool_usage: usize,
            quota_peak_non_paged_pool_usage: usize,
            quota_non_paged_pool_usage: usize,
            pagefile_usage: usize,
            peak_pagefile_usage: usize,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> isize;
            fn K32GetProcessMemoryInfo(process: isize, counters: *mut Counters, cb: u32) -> i32;
        }
        let mut c = Counters { cb: std::mem::size_of::<Counters>() as u32, ..Counters::default() };
        // SAFETY: the pseudo-handle of this process, and a struct of the size it says.
        let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) } != 0;
        ok.then(|| Usage { committed: c.pagefile_usage as u64, working_set: c.working_set_size as u64, peak_committed: c.peak_pagefile_usage as u64 })
    }
    #[cfg(not(windows))]
    {
        None
    }
}
