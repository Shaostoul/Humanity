//! The Windows job object the AI engine runs in (docs/design/embeddable-ai.md, 7.2; the runtime
//! finding, F10, quoting Microsoft's "Job Objects": "Operations performed on a job object affect all
//! processes associated with the job object. Examples include enforcing limits such as working set size
//! and process priority or terminating all processes associated with a job.").
//!
//! One job per engine start, with four limits:
//! - **kill on close**: when the last handle to the job closes, every process in it ends. The app holds
//!   the only handle, so when the app exits, or crashes, the operating system closes it and the engine
//!   goes too; nothing is left running in the background;
//! - **a memory cap** on the engine's committed memory (rung 1: the engine alone; from rung 2, a model's
//!   need from the catalog plus a margin, 9.1): past it an allocation fails, and the engine refuses the
//!   load in words instead of taking the person's memory;
//! - **below-normal priority**, enforced for the whole job, so the game's threads always win (9.2);
//! - **die on an unhandled exception**: a crashing engine ends quietly instead of putting Windows'
//!   "has stopped working" window on the person's one screen; the app notices and falls back.
//!
//! The engine is placed in the job right after it starts and before the app sends it anything; it does
//! no work before it reads its first message, so nothing runs outside the job's limits.

use std::os::windows::io::AsRawHandle;
use std::process::Child;

use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, IsProcessInJob, JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOB_OBJECT_LIMIT_PRIORITY_CLASS,
    JOB_OBJECT_LIMIT_PROCESS_MEMORY,
};
use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows::Win32::System::Threading::{GetPriorityClass, BELOW_NORMAL_PRIORITY_CLASS, CREATE_NO_WINDOW};

/// The flags the engine is started with: no console window (one would flash up on the person's screen
/// and could take focus), and below-normal priority from its first instruction.
pub const CREATION_FLAGS: u32 = CREATE_NO_WINDOW.0 | BELOW_NORMAL_PRIORITY_CLASS.0;

/// The priority class Windows reports for a below-normal process.
pub const BELOW_NORMAL: u32 = BELOW_NORMAL_PRIORITY_CLASS.0;

/// A job object holding the engine. Dropping it closes the handle, which ends the engine if it still
/// runs (kill on close).
pub struct Job {
    handle: HANDLE,
}

// SAFETY: a job handle is a kernel object handle, usable from any thread; the client moves it with the
// rest of the engine's state behind one mutex.
unsafe impl Send for Job {}

fn handle_of(child: &Child) -> HANDLE {
    HANDLE(child.as_raw_handle())
}

impl Job {
    /// A new, empty job with the four limits, its memory cap `memory_cap_mb`.
    pub fn new(memory_cap_mb: u64) -> Result<Job, String> {
        // SAFETY: no security attributes and no name: an unnamed job only this handle reaches.
        let handle = unsafe { CreateJobObjectW(None, windows::core::PCWSTR::null()) }.map_err(|e| format!("CreateJobObjectW: {e}"))?;
        let job = Job { handle };
        job.set_limits(memory_cap_mb)?;
        Ok(job)
    }

    /// Set (or change) the limits; the memory cap in MB.
    pub fn set_limits(&self, memory_cap_mb: u64) -> Result<(), String> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_PROCESS_MEMORY | JOB_OBJECT_LIMIT_PRIORITY_CLASS | JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION;
        info.BasicLimitInformation.PriorityClass = BELOW_NORMAL;
        info.ProcessMemoryLimit = usize::try_from(memory_cap_mb.saturating_mul(1024 * 1024)).unwrap_or(usize::MAX);
        // SAFETY: the pointer and size describe `info`, which lives across the call.
        unsafe {
            SetInformationJobObject(
                self.handle,
                JobObjectExtendedLimitInformation,
                &info as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        }
        .map_err(|e| format!("SetInformationJobObject: {e}"))
    }

    /// Put the engine in the job.
    pub fn assign(&self, child: &Child) -> Result<(), String> {
        // SAFETY: both handles are open (the child's for as long as `child` lives).
        unsafe { AssignProcessToJobObject(self.handle, handle_of(child)) }.map_err(|e| format!("AssignProcessToJobObject: {e}"))
    }

    /// Whether the engine is in this job.
    pub fn holds(&self, child: &Child) -> bool {
        let mut result = windows::core::BOOL(0);
        // SAFETY: both handles are open; `result` is written by the call.
        unsafe { IsProcessInJob(handle_of(child), Some(self.handle), &mut result) }.is_ok() && result.as_bool()
    }

    /// The most memory the engine has committed since it started, in MB.
    pub fn peak_mb(&self) -> Option<f64> {
        let mut info = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        // SAFETY: the pointer and size describe `info`.
        let ok = unsafe {
            QueryInformationJobObject(
                Some(self.handle),
                JobObjectExtendedLimitInformation,
                &mut info as *mut _ as *mut core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                None,
            )
        };
        ok.ok().map(|_| info.PeakProcessMemoryUsed as f64 / (1024.0 * 1024.0))
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        // SAFETY: the handle is ours and closed once.
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// The engine's committed memory now, in MB (what the job's cap counts).
pub fn memory_mb(child: &Child) -> Option<f64> {
    let mut c = PROCESS_MEMORY_COUNTERS { cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32, ..Default::default() };
    // SAFETY: the child's handle is open; `c` and its size are ours.
    let ok = unsafe { K32GetProcessMemoryInfo(handle_of(child), &mut c, c.cb) };
    ok.as_bool().then(|| c.PagefileUsage as f64 / (1024.0 * 1024.0))
}

/// The engine's priority class (for the Dev page and the tests): [`BELOW_NORMAL`] when all is well.
pub fn priority_class(child: &Child) -> u32 {
    // SAFETY: the child's handle is open.
    unsafe { GetPriorityClass(handle_of(child)) }
}
