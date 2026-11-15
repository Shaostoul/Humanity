//! hos-stackdump: every thread of a running process, by name, with its call stack.
//!
//! WHY (2026-11-14). A frozen program is one thread dump away from an answer. On 2026-11-01 the
//! full lib-test run froze silently for two hours, and scripts/lib-tests.js could only name the
//! tests that were in flight (libtest prints nothing while it joins a thread that cannot exit);
//! the GPU-hang investigation of 2026-11-14 wrote a dumper like this one in its scratchpad and
//! recommended it to the dev tooling. scripts/lib-tests.js runs this on a test program that has
//! gone quiet, before stopping it, and saves the dump beside the run's log (scripts/lib/stack-dump.js).
//! libtest names each test's thread after its test, so the dump says which test is stuck where.
//!
//! Usage:  hos-stackdump <pid>
//! Prints to stdout: one header line, then each thread as `thread <id> "<name>":` followed by its
//! frames, innermost first, as `    <module>!<function>+0x<offset>  (<file>:<line>)` when the
//! program's debug file (PDB) has them, `    <module>+0x<offset>` when it does not.
//! Exit 0 when the process was read (even if some threads could not be), 1 when it could not be
//! opened, 2 on bad arguments.
//!
//! HOW. dbghelp's StackWalk64 over each thread, suspended only for the moment of its walk and
//! resumed at once. Symbols come ONLY from the program's own folder (SymInitializeW is given that
//! folder as its search path, so _NT_SYMBOL_PATH and any symbol server are never consulted: no
//! download, ever). It runs itself at below-normal priority (the share guard's rule: nothing
//! above normal, and dev tools below it), and reads the target, never writes it.
//!
//! x86-64 Windows only (the dev PC); elsewhere it says so and exits 1.
//! Built standalone with plain rustc, std only, like the build slot (scripts/lib/stack-dump.js).
//! Its test: scripts/tests/stack-dump.test.js.

#[cfg(all(windows, target_arch = "x86_64"))]
mod win {
    use std::ffi::c_void;
    pub type Handle = *mut c_void;

    pub const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
    pub const PROCESS_VM_READ: u32 = 0x0010;
    pub const THREAD_GET_CONTEXT: u32 = 0x0008;
    pub const THREAD_SUSPEND_RESUME: u32 = 0x0002;
    pub const THREAD_QUERY_LIMITED_INFORMATION: u32 = 0x0800;
    pub const TH32CS_SNAPTHREAD: u32 = 0x0000_0004;
    pub const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
    pub const IMAGE_FILE_MACHINE_AMD64: u32 = 0x8664;
    /// CONTEXT_AMD64 | CONTROL | INTEGER | FLOATING_POINT
    pub const CONTEXT_FULL_AMD64: u32 = 0x0010_000B;
    pub const ADDR_MODE_FLAT: u32 = 3;
    pub const SYMOPT_UNDNAME: u32 = 0x0000_0002;
    pub const SYMOPT_DEFERRED_LOADS: u32 = 0x0000_0004;
    pub const SYMOPT_LOAD_LINES: u32 = 0x0000_0010;
    pub const SYMOPT_FAIL_CRITICAL_ERRORS: u32 = 0x0000_0200;
    pub const SYMOPT_NO_PROMPTS: u32 = 0x0008_0000;

    #[repr(C)]
    pub struct ThreadEntry32 {
        pub size: u32,
        pub usage: u32,
        pub thread_id: u32,
        pub owner_process_id: u32,
        pub base_pri: i32,
        pub delta_pri: i32,
        pub flags: u32,
    }

    /// The x64 CONTEXT record (1232 bytes, 16-byte aligned), as raw bytes: only ContextFlags is
    /// written and Rip, Rsp and Rbp read, at their documented offsets.
    #[repr(C, align(16))]
    pub struct Context(pub [u8; 1232]);
    pub const CTX_FLAGS: usize = 0x30;
    pub const CTX_RSP: usize = 0x98;
    pub const CTX_RBP: usize = 0xA0;
    pub const CTX_RIP: usize = 0xF8;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Address64 {
        pub offset: u64,
        pub segment: u16,
        pub mode: u32,
    }

    /// STACKFRAME64, its trailing KDHELP64 given more room than any SDK version of it needs
    /// (only the leading fields are ever set here).
    #[repr(C)]
    pub struct StackFrame64 {
        pub pc: Address64,
        pub ret: Address64,
        pub frame: Address64,
        pub stack: Address64,
        pub bstore: Address64,
        pub func_table_entry: *mut c_void,
        pub params: [u64; 4],
        pub far: i32,
        pub virtual_: i32,
        pub reserved: [u64; 3],
        pub kd_help: [u64; 40],
    }

    /// SYMBOL_INFOW; the name continues past `name` into the buffer it is allocated in.
    #[repr(C)]
    pub struct SymbolInfoW {
        pub size_of_struct: u32,
        pub type_index: u32,
        pub reserved: [u64; 2],
        pub index: u32,
        pub size: u32,
        pub mod_base: u64,
        pub flags: u32,
        pub value: u64,
        pub address: u64,
        pub register: u32,
        pub scope: u32,
        pub tag: u32,
        pub name_len: u32,
        pub max_name_len: u32,
        pub name: [u16; 1],
    }

    #[repr(C)]
    pub struct ImagehlpLineW64 {
        pub size_of_struct: u32,
        pub key: *mut c_void,
        pub line_number: u32,
        pub file_name: *mut u16,
        pub address: u64,
    }

    pub type FnTableAccess = unsafe extern "system" fn(Handle, u64) -> *mut c_void;
    pub type GetModuleBase = unsafe extern "system" fn(Handle, u64) -> u64;

    #[link(name = "kernel32")]
    extern "system" {
        pub fn OpenProcess(access: u32, inherit: i32, pid: u32) -> Handle;
        pub fn OpenThread(access: u32, inherit: i32, tid: u32) -> Handle;
        pub fn CloseHandle(h: Handle) -> i32;
        pub fn GetLastError() -> u32;
        pub fn GetCurrentProcess() -> Handle;
        pub fn SetPriorityClass(h: Handle, class: u32) -> i32;
        pub fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> Handle;
        pub fn Thread32First(snap: Handle, entry: *mut ThreadEntry32) -> i32;
        pub fn Thread32Next(snap: Handle, entry: *mut ThreadEntry32) -> i32;
        pub fn SuspendThread(h: Handle) -> u32;
        pub fn ResumeThread(h: Handle) -> u32;
        pub fn GetThreadContext(h: Handle, ctx: *mut Context) -> i32;
        pub fn GetThreadDescription(h: Handle, desc: *mut *mut u16) -> i32;
        pub fn LocalFree(mem: *mut c_void) -> *mut c_void;
        pub fn QueryFullProcessImageNameW(h: Handle, flags: u32, name: *mut u16, size: *mut u32) -> i32;
        pub fn K32GetModuleBaseNameW(h: Handle, module: Handle, name: *mut u16, size: u32) -> u32;
    }

    #[link(name = "dbghelp")]
    extern "system" {
        pub fn SymSetOptions(options: u32) -> u32;
        pub fn SymInitializeW(h: Handle, search_path: *const u16, invade: i32) -> i32;
        pub fn SymCleanup(h: Handle) -> i32;
        pub fn SymFunctionTableAccess64(h: Handle, addr: u64) -> *mut c_void;
        pub fn SymGetModuleBase64(h: Handle, addr: u64) -> u64;
        pub fn SymFromAddrW(h: Handle, addr: u64, displacement: *mut u64, symbol: *mut SymbolInfoW) -> i32;
        pub fn SymGetLineFromAddrW64(h: Handle, addr: u64, displacement: *mut u32, line: *mut ImagehlpLineW64) -> i32;
        pub fn StackWalk64(
            machine: u32,
            process: Handle,
            thread: Handle,
            frame: *mut StackFrame64,
            context: *mut Context,
            read_memory: *const c_void,
            function_table_access: Option<FnTableAccess>,
            get_module_base: Option<GetModuleBase>,
            translate_address: *const c_void,
        ) -> i32;
    }
}

/// A NUL-terminated wide string, read up to `max` characters.
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn wide(p: *const u16, max: usize) -> String {
    if p.is_null() {
        return String::new();
    }
    let mut n = 0;
    while n < max && *p.add(n) != 0 {
        n += 1;
    }
    String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Every thread id of `pid`, from a snapshot of the system's threads.
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn thread_ids(pid: u32) -> Vec<u32> {
    use win::*;
    let snap = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0);
    let mut out = Vec::new();
    if snap.is_null() || snap as isize == -1 {
        return out;
    }
    let mut te: ThreadEntry32 = std::mem::zeroed();
    te.size = std::mem::size_of::<ThreadEntry32>() as u32;
    if Thread32First(snap, &mut te) != 0 {
        loop {
            if te.owner_process_id == pid {
                out.push(te.thread_id);
            }
            if Thread32Next(snap, &mut te) == 0 {
                break;
            }
        }
    }
    CloseHandle(snap);
    out
}

/// One frame as a line: module!function+offset (file:line), as much as the symbols give.
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn frame_line(proc_h: win::Handle, addr: u64) -> String {
    use win::*;
    let base = SymGetModuleBase64(proc_h, addr);
    let module = if base != 0 {
        let mut buf = [0u16; 260];
        let n = K32GetModuleBaseNameW(proc_h, base as Handle, buf.as_mut_ptr(), buf.len() as u32) as usize;
        String::from_utf16_lossy(&buf[..n.min(buf.len())])
    } else {
        "?".to_string()
    };
    // SYMBOL_INFOW with room for a 1024-character name.
    let mut buf = vec![0u64; (std::mem::size_of::<SymbolInfoW>() + 2 * 1024 + 7) / 8];
    let sym = buf.as_mut_ptr() as *mut SymbolInfoW;
    (*sym).size_of_struct = std::mem::size_of::<SymbolInfoW>() as u32;
    (*sym).max_name_len = 1024;
    let mut disp = 0u64;
    let what = if SymFromAddrW(proc_h, addr, &mut disp, sym) != 0 {
        let n = ((*sym).name_len as usize).min(1024);
        format!("{}!{}+0x{:x}", module, String::from_utf16_lossy(std::slice::from_raw_parts((*sym).name.as_ptr(), n)), disp)
    } else if base != 0 {
        format!("{}+0x{:x}", module, addr - base)
    } else {
        format!("0x{addr:x}")
    };
    let mut line: ImagehlpLineW64 = std::mem::zeroed();
    line.size_of_struct = std::mem::size_of::<ImagehlpLineW64>() as u32;
    let mut ldisp = 0u32;
    if SymGetLineFromAddrW64(proc_h, addr, &mut ldisp, &mut line) != 0 {
        let file = wide(line.file_name, 1024);
        // The last two parts of the path are enough to find it.
        let short: Vec<&str> = file.rsplit(['\\', '/']).take(2).collect();
        let short: Vec<&str> = short.into_iter().rev().collect();
        format!("{what}  ({}:{})", short.join("/"), line.line_number)
    } else {
        what
    }
}

/// One thread's name and frames (the thread suspended only while its stack is walked).
#[cfg(all(windows, target_arch = "x86_64"))]
unsafe fn thread_report(proc_h: win::Handle, tid: u32) -> (String, Vec<String>) {
    use win::*;
    let th = OpenThread(THREAD_GET_CONTEXT | THREAD_SUSPEND_RESUME | THREAD_QUERY_LIMITED_INFORMATION, 0, tid);
    if th.is_null() {
        return (String::new(), vec![format!("    (could not open the thread: error {})", GetLastError())]);
    }
    let mut name_p: *mut u16 = std::ptr::null_mut();
    let name = if GetThreadDescription(th, &mut name_p) >= 0 && !name_p.is_null() {
        let n = wide(name_p, 512);
        LocalFree(name_p as *mut _);
        n
    } else {
        String::new()
    };
    let mut lines = Vec::new();
    // Walk addresses first, symbolize after the thread is resumed: the walk is what needs it still.
    let mut pcs: Vec<u64> = Vec::new();
    if SuspendThread(th) == u32::MAX {
        lines.push(format!("    (could not suspend the thread: error {})", GetLastError()));
    } else {
        let mut ctx = Context([0u8; 1232]);
        ctx.0[CTX_FLAGS..CTX_FLAGS + 4].copy_from_slice(&CONTEXT_FULL_AMD64.to_le_bytes());
        if GetThreadContext(th, &mut ctx) != 0 {
            let reg = |off: usize| u64::from_le_bytes(ctx.0[off..off + 8].try_into().unwrap());
            let mut frame: StackFrame64 = std::mem::zeroed();
            frame.pc = Address64 { offset: reg(CTX_RIP), segment: 0, mode: ADDR_MODE_FLAT };
            frame.frame = Address64 { offset: reg(CTX_RBP), segment: 0, mode: ADDR_MODE_FLAT };
            frame.stack = Address64 { offset: reg(CTX_RSP), segment: 0, mode: ADDR_MODE_FLAT };
            for _ in 0..128 {
                let ok = StackWalk64(
                    IMAGE_FILE_MACHINE_AMD64,
                    proc_h,
                    th,
                    &mut frame,
                    &mut ctx,
                    std::ptr::null(),
                    Some(SymFunctionTableAccess64),
                    Some(SymGetModuleBase64),
                    std::ptr::null(),
                );
                if ok == 0 || frame.pc.offset == 0 {
                    break;
                }
                pcs.push(frame.pc.offset);
            }
        } else {
            lines.push(format!("    (could not read the thread's registers: error {})", GetLastError()));
        }
        ResumeThread(th);
    }
    CloseHandle(th);
    for pc in pcs {
        lines.push(format!("    {}", frame_line(proc_h, pc)));
    }
    (name, lines)
}

#[cfg(all(windows, target_arch = "x86_64"))]
fn run(pid: u32) -> i32 {
    use win::*;
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
        let proc_h = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
        if proc_h.is_null() {
            eprintln!("hos-stackdump: cannot open process {pid} (error {})", GetLastError());
            return 1;
        }
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let exe = if QueryFullProcessImageNameW(proc_h, 0, buf.as_mut_ptr(), &mut len) != 0 { String::from_utf16_lossy(&buf[..len as usize]) } else { String::new() };
        // Symbols from the program's own folder only: never _NT_SYMBOL_PATH, never a server.
        let folder = exe.rsplit_once('\\').map(|(d, _)| d.to_string()).unwrap_or_default();
        let search = to_wide(&folder);
        SymSetOptions(SYMOPT_UNDNAME | SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES | SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_NO_PROMPTS);
        let sym_ok = SymInitializeW(proc_h, search.as_ptr(), 1) != 0;
        let tids = thread_ids(pid);
        println!("hos-stackdump: process {pid} ({exe}), {} thread(s){}", tids.len(), if sym_ok { "" } else { "; symbols could not be loaded, addresses only" });
        for tid in tids {
            let (name, lines) = thread_report(proc_h, tid);
            println!();
            println!("thread {tid} \"{name}\":");
            for l in lines {
                println!("{l}");
            }
        }
        if sym_ok {
            SymCleanup(proc_h);
        }
        CloseHandle(proc_h);
    }
    0
}

#[cfg(not(all(windows, target_arch = "x86_64")))]
fn run(_pid: u32) -> i32 {
    eprintln!("hos-stackdump: x86-64 Windows only");
    1
}

fn main() {
    let pid = match std::env::args().nth(1).and_then(|a| a.parse::<u32>().ok()) {
        Some(p) => p,
        None => {
            eprintln!("usage: hos-stackdump <pid>");
            std::process::exit(2);
        }
    };
    std::process::exit(run(pid));
}
