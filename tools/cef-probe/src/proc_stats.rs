//! CPU time and memory for the WHOLE browser host, not just this process.
//!
//! A Chromium embedding is never one process. Chromium relaunches this same
//! executable several times over: a renderer for the page, a GPU process, a
//! network service, a utility process or two. Measuring only the process that
//! called `main` would report a fraction of the real cost and would make the
//! spike's answer wrong in the flattering direction.
//!
//! So this module walks the machine's process list, picks out every process
//! running the same executable file name as us, and sums their processor time
//! and memory. Chromium's children are relaunches of our own exe, so matching
//! on the name catches exactly the family we started and nothing else. (If a
//! second copy of the probe were running at the same time it would be counted
//! too, which is why the measurement scripts run one at a time.)

use windows_sys::Win32::Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::Threading::{
    GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_VM_READ,
};

/// One reading of the whole process family.
#[derive(Default, Clone, Copy)]
pub struct TreeSample {
    /// Total processor time used since each process started, in 100-nanosecond
    /// units (the unit Windows reports). User time plus kernel time.
    pub cpu_100ns: u64,
    /// Sum of every process's working set (physical memory in use).
    pub working_set: u64,
    /// Sum of every process's private commit (memory not shared with others).
    pub private: u64,
    /// How many processes were counted.
    pub process_count: u32,
}

/// Take a reading. Two readings a known time apart give the CPU load: the
/// difference in processor time divided by the wall time is how many cores'
/// worth of work the family did.
pub fn sample_tree() -> TreeSample {
    let mut out = TreeSample::default();
    let Some(our_name) = current_exe_file_name() else {
        return out;
    };

    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if snap == INVALID_HANDLE_VALUE {
        return out;
    }
    let mut entry: PROCESSENTRY32W = unsafe { std::mem::zeroed() };
    entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;

    let mut ok = unsafe { Process32FirstW(snap, &mut entry) };
    while ok != 0 {
        let name = wide_to_string(&entry.szExeFile);
        if name.eq_ignore_ascii_case(&our_name) {
            accumulate(entry.th32ProcessID, &mut out);
        }
        ok = unsafe { Process32NextW(snap, &mut entry) };
    }
    unsafe { CloseHandle(snap) };
    out
}

fn accumulate(pid: u32, out: &mut TreeSample) {
    let handle = unsafe {
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, 0, pid)
    };
    if handle.is_null() {
        return;
    }
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    if unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) } != 0 {
        out.cpu_100ns += filetime_to_u64(kernel) + filetime_to_u64(user);
    }
    let mut mem: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    mem.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    if unsafe { GetProcessMemoryInfo(handle, &mut mem, mem.cb) } != 0 {
        out.working_set += mem.WorkingSetSize as u64;
        // PagefileUsage is the private commit: memory this process asked for
        // that is not shared with any other.
        out.private += mem.PagefileUsage as u64;
    }
    out.process_count += 1;
    unsafe { CloseHandle(handle) };
}

fn filetime_to_u64(ft: FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | ft.dwLowDateTime as u64
}

fn current_exe_file_name() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    Some(exe.file_name()?.to_string_lossy().into_owned())
}

/// Windows hands back a fixed-size UTF-16 buffer with the name at the front
/// and a NUL after it.
fn wide_to_string(buf: &[u16]) -> String {
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}
