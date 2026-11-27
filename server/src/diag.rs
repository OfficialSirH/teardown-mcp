//! Diagnostics from outside the game process: health, process counters, crash reports, dumps.
//! Nothing here goes through the game's scripts, so it works while the game is hung, paused,
//! in a menu or gone.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use serde_json::{json, Value};
use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

type HANDLE = *mut c_void;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const PROCESS_QUERY_INFORMATION: u32 = 0x0400;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const PROCESS_VM_READ: u32 = 0x0010;
const PROCESS_DUP_HANDLE: u32 = 0x0040;
const SYNCHRONIZE: u32 = 0x0010_0000;
const THREAD_SUSPEND_RESUME: u32 = 0x0002;
const THREAD_GET_CONTEXT: u32 = 0x0008;
const THREAD_QUERY_INFORMATION: u32 = 0x0040;
const STILL_ACTIVE: u32 = 259;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct FILETIME {
    lo: u32,
    hi: u32,
}
fn ft(f: &FILETIME) -> u64 {
    ((f.hi as u64) << 32) | f.lo as u64
}

#[repr(C)]
#[derive(Default)]
struct PMC {
    cb: u32,
    page_faults: u32,
    peak_working_set: usize,
    working_set: usize,
    quota_peak_paged: usize,
    quota_paged: usize,
    quota_peak_nonpaged: usize,
    quota_nonpaged: usize,
    pagefile: usize,
    peak_pagefile: usize,
    private: usize,
}

#[repr(C)]
struct THREADENTRY32 {
    size: u32,
    usage: u32,
    tid: u32,
    owner_pid: u32,
    base_pri: i32,
    delta_pri: i32,
    flags: u32,
}

#[repr(C, align(16))]
struct Context([u8; 1232]);

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> HANDLE;
    fn CloseHandle(h: HANDLE) -> i32;
    fn GetProcessTimes(h: HANDLE, c: *mut FILETIME, e: *mut FILETIME, k: *mut FILETIME, u: *mut FILETIME) -> i32;
    fn GetThreadTimes(h: HANDLE, c: *mut FILETIME, e: *mut FILETIME, k: *mut FILETIME, u: *mut FILETIME) -> i32;
    fn K32GetProcessMemoryInfo(h: HANDLE, p: *mut PMC, cb: u32) -> i32;
    fn GetExitCodeProcess(h: HANDLE, code: *mut u32) -> i32;
    fn QueryFullProcessImageNameW(h: HANDLE, flags: u32, buf: *mut u16, size: *mut u32) -> i32;
    fn GetSystemTimeAsFileTime(f: *mut FILETIME);
    fn CreateToolhelp32Snapshot(flags: u32, pid: u32) -> HANDLE;
    fn Thread32First(h: HANDLE, e: *mut THREADENTRY32) -> i32;
    fn Thread32Next(h: HANDLE, e: *mut THREADENTRY32) -> i32;
    fn OpenThread(access: u32, inherit: i32, tid: u32) -> HANDLE;
    fn SuspendThread(h: HANDLE) -> u32;
    fn ResumeThread(h: HANDLE) -> u32;
    fn GetThreadContext(h: HANDLE, ctx: *mut Context) -> i32;
    fn K32GetModuleBaseNameW(p: HANDLE, m: *mut c_void, buf: *mut u16, n: u32) -> u32;
    fn K32GetModuleFileNameExW(p: HANDLE, m: *mut c_void, buf: *mut u16, n: u32) -> u32;
    fn K32EnumProcessModules(p: HANDLE, mods: *mut *mut c_void, cb: u32, needed: *mut u32) -> i32;
    fn ReadProcessMemory(p: HANDLE, addr: *const c_void, buf: *mut c_void, size: usize, read: *mut usize) -> i32;
    fn GetProcessHandleCount(h: HANDLE, n: *mut u32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> HANDLE;
    fn GetWindowThreadProcessId(h: HANDLE, pid: *mut u32) -> u32;
    fn IsHungAppWindow(h: HANDLE) -> i32;
}

#[link(name = "dbghelp")]
extern "system" {
    fn MiniDumpWriteDump(p: HANDLE, pid: u32, file: HANDLE, ty: u32, e: *const c_void, u: *const c_void, c: *const c_void) -> i32;
    fn SymSetOptions(o: u32) -> u32;
    fn SymInitializeW(p: HANDLE, path: *const u16, invade: i32) -> i32;
    fn SymCleanup(p: HANDLE) -> i32;
    fn StackWalk64(machine: u32, p: HANDLE, t: HANDLE, frame: *mut u8, ctx: *mut Context, read: *const c_void, fta: *const c_void, gmb: *const c_void, tr: *const c_void) -> i32;
    fn SymFunctionTableAccess64(p: HANDLE, addr: u64) -> *mut c_void;
    fn SymGetModuleBase64(p: HANDLE, addr: u64) -> u64;
    fn SymFromAddr(p: HANDLE, addr: u64, disp: *mut u64, sym: *mut u8) -> i32;
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

const TITLES: [&str; 3] = ["TD - Host", "TD - Client1", "Teardown"];

/// A game window: its title, process id and handle.
pub struct Game {
    pub title: String,
    pub pid: u32,
    hwnd: HANDLE,
    pub exe: String,
}

fn exe_name(pid: u32) -> String {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return String::new();
        }
        let mut buf = [0u16; 520];
        let mut n = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, 0, buf.as_mut_ptr(), &mut n);
        CloseHandle(h);
        if ok == 0 {
            return String::new();
        }
        let full = String::from_utf16_lossy(&buf[..n as usize]);
        full.rsplit('\\').next().unwrap_or("").to_string()
    }
}

/// Checked before the first bridge command after a quiet spell: is there a game window the
/// bridge can talk to (the multiplayer test's host, or the normal game), and is it responding?
/// Cheap (window lookup only), so a closed or hung game is reported at once instead of after
/// the bridge's response timeout.
pub fn alive_check() -> Result<(), String> {
    let gs: Vec<Game> = games(None).into_iter().filter(|g| g.title != "TD - Client1").collect();
    let Some(g) = gs.first() else {
        return Err(if games(Some("TD - Client1")).is_empty() {
            "Teardown is not running: no game window (\"Teardown\" or the multiplayer test's \"TD - Host\") is open. Ask the user to start the game and load a level, or start a multiplayer test with mp_test_start".to_string()
        } else {
            "only the multiplayer test's client window (\"TD - Client1\") is open, no host: restart the test with mp_test_start and restart=true".to_string()
        });
    };
    if unsafe { IsHungAppWindow(g.hwnd) } != 0 {
        return Err(format!(
            "the game window \"{}\" is not responding (Windows sees it as hung). Use game_health and dump_game to see where it is stuck",
            g.title
        ));
    }
    Ok(())
}

/// Checked while waiting for an answer (once a second, so window lookups only): are the
/// windows that must answer still open? `remote`: the multiplayer test's client too. A window
/// that closed is reported with its process's exit code when this server saw it running.
pub fn windows_check(remote: bool) -> Result<(), String> {
    if games(None).iter().all(|g| g.title == "TD - Client1") {
        return Err(match exit_of(&["TD - Host", "Teardown"]) {
            Some((title, pid, code)) => format!(
                "the game window \"{title}\" (pid {pid}) has closed: exit code {code:#010x}{}",
                meaning_suffix(code)
            ),
            None => "no game window (\"TD - Host\" or \"Teardown\") is open".to_string(),
        });
    }
    if remote && games(Some("TD - Client1")).is_empty() {
        let why = match exit_of(&["TD - Client1"]) {
            Some((_, pid, code)) => format!(
                "the multiplayer test's client (\"TD - Client1\", pid {pid}) has exited with {code:#010x}{}",
                meaning_suffix(code)
            ),
            None => "the multiplayer test's client window (\"TD - Client1\") is not open".to_string(),
        };
        return Err(format!(
            "{why}, so side=remote has no game to run in. read_log with file log_1.txt shows the client's log, crash_report the game's own crash report if it wrote one; mp_test_join starts a new client (the host keeps running)"
        ));
    }
    Ok(())
}

fn meaning_suffix(code: u32) -> String {
    match exit_meaning(code) {
        "" => String::new(),
        m => format!(" ({m})"),
    }
}

/// The most recently seen ended process that had one of these window titles, without
/// forgetting it (game_health and crash_report still report it). The window goes a moment
/// before the process ends, so this waits up to a second for that.
fn exit_of(titles: &[&str]) -> Option<(String, u32, u32)> {
    for _ in 0..10 {
        if let Some(x) = exit_of_now(titles) {
            return Some(x);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    None
}

/// Keep an eye on a process this server started (before it has a window), so its exit can be
/// told and its exit code read.
pub fn watch(pid: u32, title: &str) {
    remember(pid, title, "teardown_modtest.exe");
}

/// Exit code of a watched process that has ended; None while it runs (or if never watched).
pub fn exit_code(pid: u32) -> Option<u32> {
    let seen = SEEN.lock().unwrap();
    let s = seen.iter().find(|s| s.pid == pid)?;
    let mut code = STILL_ACTIVE;
    unsafe { GetExitCodeProcess(s.handle as HANDLE, &mut code) };
    (code != STILL_ACTIVE).then_some(code)
}

pub fn exit_text(code: u32) -> String {
    format!("{code:#010x}{}", meaning_suffix(code))
}

fn exit_of_now(titles: &[&str]) -> Option<(String, u32, u32)> {
    let seen = SEEN.lock().unwrap();
    seen.iter()
        .filter(|s| titles.contains(&s.title.as_str()))
        .filter_map(|s| {
            let mut code = STILL_ACTIVE;
            unsafe { GetExitCodeProcess(s.handle as HANDLE, &mut code) };
            (code != STILL_ACTIVE).then(|| (s.since, (s.title.clone(), s.pid, code)))
        })
        .max_by_key(|(since, _)| *since)
        .map(|(_, x)| x)
}

/// Game windows, found by title; a window only counts when its process is a Teardown exe
/// (a File Explorer window on the "Teardown" folder has the same title).
pub fn games(only: Option<&str>) -> Vec<Game> {
    let titles: Vec<&str> = match only {
        Some(t) => vec![t],
        None => TITLES.to_vec(),
    };
    let mut out = Vec::new();
    for t in titles {
        let w = wide(t);
        let hwnd = unsafe { FindWindowW(std::ptr::null(), w.as_ptr()) };
        if hwnd.is_null() {
            continue;
        }
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
        let exe = exe_name(pid);
        if !exe.to_lowercase().starts_with("teardown") {
            continue;
        }
        remember(pid, t, &exe);
        out.push(Game { title: t.to_string(), pid, hwnd, exe });
    }
    out
}

/// Processes seen by any diagnostic call, kept open so their exit code can be read once they end.
struct Seen {
    pid: u32,
    title: String,
    exe: String,
    handle: usize,
    since: Instant,
}
static SEEN: Mutex<Vec<Seen>> = Mutex::new(Vec::new());

fn remember(pid: u32, title: &str, exe: &str) {
    let mut seen = SEEN.lock().unwrap();
    // A starting test client's window is titled "Teardown" before "TD - Client1": keep the
    // latest title, which is the one an exit is looked up by.
    if let Some(s) = seen.iter_mut().find(|s| s.pid == pid) {
        if s.title != title {
            s.title = title.into();
        }
        return;
    }
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid) };
    if !h.is_null() {
        seen.push(Seen { pid, title: title.into(), exe: exe.into(), handle: h as usize, since: Instant::now() });
    }
}

pub fn exit_meaning(code: u32) -> &'static str {
    match code {
        0 => "normal exit",
        1 => "ended with code 1 (also what a forced close by taskkill or the multiplayer test's stop gives)",
        0xC000_0005 => "access violation (a native crash)",
        0xC000_00FD => "stack overflow",
        0xC000_0409 => "stack buffer overrun / fail-fast",
        0xC000_0374 => "heap corruption",
        0xC000_001D => "illegal instruction",
        0xC000_0094 => "integer divide by zero",
        0x8000_0003 => "breakpoint",
        0xE06D_7363 => "unhandled C++ exception",
        0xC000_013A => "closed with Ctrl+C / console close",
        _ => "",
    }
}

/// Processes seen earlier that have ended since: (title, exe, pid, exit code, seconds watched).
fn ended() -> Vec<Value> {
    let mut seen = SEEN.lock().unwrap();
    let mut out = Vec::new();
    seen.retain(|s| {
        let mut code = STILL_ACTIVE;
        unsafe { GetExitCodeProcess(s.handle as HANDLE, &mut code) };
        if code == STILL_ACTIVE {
            return true;
        }
        out.push(json!({
            "window": s.title, "exe": s.exe, "pid": s.pid,
            "exit_code": format!("{code:#010x}"), "meaning": exit_meaning(code),
            "first_seen_seconds_ago": s.since.elapsed().as_secs()
        }));
        unsafe { CloseHandle(s.handle as HANDLE) };
        false
    });
    out
}

/// CPU time (100 ns units) and creation time of a process.
fn times(h: HANDLE) -> Option<(u64, u64)> {
    let (mut c, mut e, mut k, mut u) = Default::default();
    if unsafe { GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) } == 0 {
        return None;
    }
    Some((ft(&k) + ft(&u), ft(&c)))
}

fn now_ft() -> u64 {
    let mut f = FILETIME::default();
    unsafe { GetSystemTimeAsFileTime(&mut f) };
    ft(&f)
}

fn mb(b: usize) -> f64 {
    (b as f64 / 1048576.0 * 10.0).round() / 10.0
}

fn threads_of(pid: u32) -> Vec<u32> {
    let mut out = Vec::new();
    unsafe {
        let snap = CreateToolhelp32Snapshot(0x4, 0); // TH32CS_SNAPTHREAD
        if snap as isize == -1 {
            return out;
        }
        let mut e = THREADENTRY32 { size: std::mem::size_of::<THREADENTRY32>() as u32, usage: 0, tid: 0, owner_pid: 0, base_pri: 0, delta_pri: 0, flags: 0 };
        let mut ok = Thread32First(snap, &mut e);
        while ok != 0 {
            if e.owner_pid == pid {
                out.push(e.tid);
            }
            ok = Thread32Next(snap, &mut e);
        }
        CloseHandle(snap);
    }
    out
}

/// A snapshot of the process counters, for differences over time.
pub struct Counters {
    cpu: u64,
    at: Instant,
    pub working_set: usize,
    pub private: usize,
    peak_working_set: usize,
    created: u64,
    handles: u32,
}

pub fn counters(pid: u32) -> Option<Counters> {
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let (cpu, created) = times(h).unwrap_or((0, 0));
        let mut m = PMC { cb: std::mem::size_of::<PMC>() as u32, ..Default::default() };
        K32GetProcessMemoryInfo(h, &mut m, m.cb);
        let mut handles = 0;
        GetProcessHandleCount(h, &mut handles);
        CloseHandle(h);
        Some(Counters { cpu, at: Instant::now(), working_set: m.working_set, private: m.private, peak_working_set: m.peak_working_set, created, handles })
    }
}

/// CPU use between two snapshots: (percent of one core, percent of the whole machine).
pub fn cpu_between(a: &Counters, b: &Counters) -> (f64, f64) {
    let wall = b.at.duration_since(a.at).as_secs_f64().max(0.001);
    let used = (b.cpu.saturating_sub(a.cpu)) as f64 / 1e7;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let one = used / wall * 100.0;
    ((one * 10.0).round() / 10.0, (one / cores * 10.0).round() / 10.0)
}

fn process_json(g: &Game, a: &Counters, b: &Counters) -> Value {
    let (core, all) = cpu_between(a, b);
    let responding = unsafe { IsHungAppWindow(g.hwnd) } == 0;
    let uptime = if b.created > 0 { (now_ft().saturating_sub(b.created)) / 10_000_000 } else { 0 };
    json!({
        "window": g.title, "exe": g.exe, "pid": g.pid,
        "responding": responding,
        "uptime_seconds": uptime,
        "cpu_percent_of_one_core": core,
        "cpu_percent_of_machine": all,
        "working_set_mb": mb(b.working_set),
        "private_mb": mb(b.private),
        "peak_working_set_mb": mb(b.peak_working_set),
        "threads": threads_of(g.pid).len(),
        "handles": b.handles,
    })
}

/// GPU load and memory from nvidia-smi, if it is installed.
pub fn gpu() -> Value {
    let out = Command::new("nvidia-smi")
        .args(["--query-gpu=name,utilization.gpu,memory.used,memory.total,temperature.gpu,power.draw", "--format=csv,noheader,nounits"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output();
    let Ok(out) = out else { return Value::Null };
    let s = String::from_utf8_lossy(&out.stdout);
    let f: Vec<&str> = s.lines().next().unwrap_or("").split(',').map(str::trim).collect();
    if f.len() < 4 {
        return Value::Null;
    }
    let num = |x: &str| x.parse::<f64>().ok();
    json!({"name": f[0], "utilization_percent": num(f[1]), "memory_used_mb": num(f[2]), "memory_total_mb": num(f[3]),
           "temperature_c": f.get(4).and_then(|x| num(x)), "power_w": f.get(5).and_then(|x| num(x)),
           "note": "whole GPU, all programs"})
}

// ---------------------------------------------------------------- crash reports

/// The game's own crash reports: <log dir>/crash/<date>_Steam_LOCAL-BUILD/, newest first.
pub fn crash_dirs(log_dir: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(log_dir.join("crash"))
        .map(|r| r.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    v.sort();
    v.reverse();
    v
}

fn read_json_lenient(p: &Path) -> Value {
    let s = std::fs::read_to_string(p).unwrap_or_default();
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

/// One line per report, for game_health.
pub fn crash_list(log_dir: &Path, max: usize) -> Vec<Value> {
    crash_dirs(log_dir)
        .iter()
        .take(max)
        .map(|d| {
            let y = read_json_lenient(&d.join("error.yaml"));
            json!({
                "report": d.file_name().map(|n| n.to_string_lossy().into_owned()),
                "message": y["message"],
                "lua_stack_frames": y["callstack_lua"].as_array().map(|a| a.len()),
            })
        })
        .collect()
}

/// A file from inside a zip, through Windows' own tar (bsdtar reads zip).
fn unzip_file(zip: &Path, name: &str) -> Option<String> {
    let out = Command::new("tar")
        .arg("-xOf")
        .arg(zip)
        .arg(name)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

pub fn crash_report(log_dir: &Path, index: usize, log_lines: usize) -> Result<Value, String> {
    let dirs = crash_dirs(log_dir);
    let d = dirs.get(index).ok_or_else(|| {
        format!("no crash report number {index}; {} report(s) in {}", dirs.len(), log_dir.join("crash").display())
    })?;
    let y = read_json_lenient(&d.join("error.yaml"));
    let active: Vec<Value> = y["active_mods"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| m.as_object())
        .flat_map(|m| m.iter())
        .filter(|(_, v)| v["active"] == json!(true))
        .map(|(k, _)| json!(k))
        .collect();
    let exe = crate::exe_names::load(&d.join("teardown.exe"));
    let native: Vec<String> = String::from_utf8_lossy(&std::fs::read(d.join("callstack.txt")).unwrap_or_default())
        .replace('\0', "")
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| {
            let bare = l.strip_prefix("0x").filter(|h| h.chars().all(|c| c.is_ascii_hexdigit()));
            match (bare.and_then(|h| u32::from_str_radix(h.trim_start_matches('0'), 16).ok()), &exe) {
                (Some(rva), Some(e)) => format!("teardown.exe+{rva:#x} {}", e.describe(rva)).trim_end().to_string(),
                (Some(rva), None) => format!("teardown.exe+{rva:#x}"),
                _ => l.to_string(),
            }
        })
        .collect();
    let log = unzip_file(&d.join("crashData.zip"), "log.txt").map(|s| {
        let lines: Vec<&str> = s.lines().collect();
        lines[lines.len().saturating_sub(log_lines)..].join("\n")
    });
    let mut other = serde_json::Map::new();
    if let Some(o) = y.as_object() {
        for (k, v) in o {
            if !["GPU", "active_mods", "callstack_lua", "message", "version", "memory"].contains(&k.as_str()) {
                other.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(json!({
        "report": d,
        "message": y["message"],
        "version": y["version"],
        "lua_callstack": y["callstack_lua"],
        "native_callstack": native,
        "exe_build_stamp": exe.as_ref().map(|e| format!("{:#x}", e.stamp)),
        "names_table": exe.as_ref().and_then(|e| e.table.map(|t| t.lines().next().unwrap_or("").trim_start_matches("# ").to_string())),
        "native_note": if exe.as_ref().map(|e| e.table.is_some()).unwrap_or(false) {
            "Innermost frame first. The crash is in the first teardown.exe frame after KiUserExceptionDispatcher. Function starts come from the copy of teardown.exe in the report; names from the table for exactly this build (names_table: where it came from; reverse engineered, not verified here). The Windows frames above it are the crash handler.".to_string()
        } else {
            format!("Innermost frame first. The crash is in the first teardown.exe frame after KiUserExceptionDispatcher. Function starts come from the copy of teardown.exe in the report; this build has no name table (tables: {}; make one with tools/port_names.py). The Windows frames above it are the crash handler.", crate::exe_names::known_builds().join(", "))
        },
        "memory": y["memory"],
        "active_mods": active,
        "other_fields": other,
        "log_tail": log,
        "files": std::fs::read_dir(d).map(|r| r.flatten().map(|e| e.file_name().to_string_lossy().into_owned()).collect::<Vec<_>>()).unwrap_or_default(),
    }))
}

// ---------------------------------------------------------------- dump and stacks

fn module_name(p: HANDLE, base: u64) -> String {
    let mut buf = [0u16; 260];
    let n = unsafe { K32GetModuleBaseNameW(p, base as *mut c_void, buf.as_mut_ptr(), buf.len() as u32) };
    if n == 0 {
        "?".into()
    } else {
        String::from_utf16_lossy(&buf[..n as usize])
    }
}

thread_local! {
    /// Function tables of the game exes seen in this dump, by module base.
    static EXES: std::cell::RefCell<std::collections::HashMap<u64, Option<crate::exe_names::Exe>>> = Default::default();
}

fn frame_name(p: HANDLE, pc: u64) -> String {
    let base = unsafe { SymGetModuleBase64(p, pc) };
    let module = if base != 0 { module_name(p, base) } else { "?".into() };
    let mut s = if base != 0 { format!("{module}+{:#x}", pc - base) } else { format!("{pc:#x}") };
    if base != 0 && module.to_lowercase().starts_with("teardown") {
        let where_ = EXES.with(|m| {
            let mut m = m.borrow_mut();
            let exe = m.entry(base).or_insert_with(|| {
                let mut buf = [0u16; 520];
                let n = unsafe { K32GetModuleFileNameExW(p, base as *mut c_void, buf.as_mut_ptr(), buf.len() as u32) };
                (n > 0).then(|| crate::exe_names::load(Path::new(&String::from_utf16_lossy(&buf[..n as usize])))).flatten()
            });
            exe.as_ref().map(|e| e.describe((pc - base) as u32)).unwrap_or_default()
        });
        if !where_.is_empty() {
            s.push(' ');
            s.push_str(&where_);
        }
    }
    // Exported names only (no symbol files): trust them only for Windows' own DLLs and when close.
    if !module.to_lowercase().starts_with("teardown") {
        let mut sym = [0u8; 88 + 256];
        sym[0..4].copy_from_slice(&88u32.to_le_bytes());
        sym[80..84].copy_from_slice(&255u32.to_le_bytes());
        let mut disp = 0u64;
        if unsafe { SymFromAddr(p, pc, &mut disp, sym.as_mut_ptr()) } != 0 && disp < 0x4000 {
            let name: Vec<u8> = sym[84..].iter().take_while(|&&c| c != 0).copied().collect();
            s.push_str(&format!(" ({}+{:#x})", String::from_utf8_lossy(&name), disp));
        }
    }
    s
}

/// Walk one thread's stack. The thread is suspended for the duration.
fn walk(p: HANDLE, tid: u32, max: usize) -> Result<Vec<String>, String> {
    unsafe {
        let t = OpenThread(THREAD_SUSPEND_RESUME | THREAD_GET_CONTEXT | THREAD_QUERY_INFORMATION, 0, tid);
        if t.is_null() {
            return Err(format!("cannot open thread {tid}"));
        }
        if SuspendThread(t) == u32::MAX {
            CloseHandle(t);
            return Err(format!("cannot suspend thread {tid}"));
        }
        let mut ctx = Context([0; 1232]);
        ctx.0[0x30..0x34].copy_from_slice(&0x0010_000Bu32.to_le_bytes()); // CONTEXT_FULL (AMD64)
        let mut frames = Vec::new();
        if GetThreadContext(t, &mut ctx) != 0 {
            let rd = |o: usize| u64::from_le_bytes(ctx.0[o..o + 8].try_into().unwrap());
            let (rip, rsp) = (rd(0xF8), rd(0x98));
            // STACKFRAME64 is written by offset: AddrPC @0, AddrFrame @32, AddrStack @48 (Offset, then Mode @+12).
            let mut sf = [0u8; 512];
            let mut set = |o: usize, v: u64| {
                sf[o..o + 8].copy_from_slice(&v.to_le_bytes());
                sf[o + 12..o + 16].copy_from_slice(&3u32.to_le_bytes()); // AddrModeFlat
            };
            set(0, rip);
            set(32, rsp);
            set(48, rsp);
            let fta = SymFunctionTableAccess64 as unsafe extern "system" fn(HANDLE, u64) -> *mut c_void as *const c_void;
            let gmb = SymGetModuleBase64 as unsafe extern "system" fn(HANDLE, u64) -> u64 as *const c_void;
            for _ in 0..max {
                if StackWalk64(0x8664, p, t, sf.as_mut_ptr(), &mut ctx, std::ptr::null(), fta, gmb, std::ptr::null()) == 0 {
                    break;
                }
                let pc = u64::from_le_bytes(sf[0..8].try_into().unwrap());
                if pc == 0 {
                    break;
                }
                frames.push(frame_name(p, pc));
            }
        }
        ResumeThread(t);
        CloseHandle(t);
        Ok(frames)
    }
}

fn thread_cpu_and_start(tid: u32) -> (u64, u64) {
    unsafe {
        let t = OpenThread(THREAD_QUERY_INFORMATION, 0, tid);
        if t.is_null() {
            return (0, u64::MAX);
        }
        let (mut c, mut e, mut k, mut u) = Default::default();
        GetThreadTimes(t, &mut c, &mut e, &mut k, &mut u);
        CloseHandle(t);
        (ft(&k) + ft(&u), ft(&c))
    }
}

/// Stacks of the main thread (the first one created) and the busiest others; optionally a
/// minidump file.
pub fn dump(g: &Game, out_dir: Option<&Path>, full: bool, other_threads: usize) -> Result<Value, String> {
    unsafe {
        let p = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ | PROCESS_DUP_HANDLE, 0, g.pid);
        if p.is_null() {
            return Err(format!("cannot open the game process {} for reading", g.pid));
        }
        SymSetOptions(0x2 | 0x4 | 0x200 | 0x80000); // undname, deferred loads, no error boxes, no prompts
        let empty = wide("");
        SymInitializeW(p, empty.as_ptr(), 1);

        let mut threads: Vec<(u32, u64, u64)> = threads_of(g.pid).into_iter().map(|t| {
            let (cpu, start) = thread_cpu_and_start(t);
            (t, cpu, start)
        }).collect();
        threads.sort_by_key(|x| x.2);
        let main = threads.first().map(|x| x.0);
        let mut result = json!({"window": g.title, "pid": g.pid, "threads": threads.len()});
        if let Some(m) = main {
            result["main_thread"] = json!({"tid": m, "cpu_seconds": threads[0].1 as f64 / 1e7, "stack": walk(p, m, 64).unwrap_or_else(|e| vec![e])});
        }
        let mut busy: Vec<_> = threads.iter().skip(1).copied().collect();
        busy.sort_by(|a, b| b.1.cmp(&a.1));
        let others: Vec<Value> = busy.iter().take(other_threads).map(|&(tid, cpu, _)| {
            json!({"tid": tid, "cpu_seconds": (cpu as f64 / 1e5).round() / 100.0, "stack": walk(p, tid, 12).unwrap_or_else(|e| vec![e])})
        }).collect();
        result["busiest_other_threads"] = json!(others);

        if let Some(dir) = out_dir {
            let _ = std::fs::create_dir_all(dir);
            let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
            let path = dir.join(format!("{}-{}-{stamp}{}.dmp", g.exe.trim_end_matches(".exe"), g.pid, if full { "-full" } else { "" }));
            let started = Instant::now();
            match std::fs::File::create(&path) {
                Ok(f) => {
                    // Thread info, unloaded modules, handles, and the memory the stacks point to; full adds all memory.
                    let ty: u32 = 0x1000 | 0x20 | 0x4 | if full { 0x2 | 0x800 } else { 0x40 | 0x10 };
                    let ok = MiniDumpWriteDump(p, g.pid, f.as_raw_handle() as HANDLE, ty, std::ptr::null(), std::ptr::null(), std::ptr::null());
                    drop(f);
                    let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                    result["dump"] = if ok != 0 {
                        json!({"path": path, "size_mb": mb(size as usize), "seconds": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
                               "open_with": "WinDbg (Microsoft Store) or Visual Studio: File > Open > the .dmp, then Debug with Native Only"})
                    } else {
                        let _ = std::fs::remove_file(&path);
                        json!({"error": "MiniDumpWriteDump failed"})
                    };
                }
                Err(e) => result["dump"] = json!({"error": format!("cannot create {}: {e}", path.display())}),
            }
        }
        SymCleanup(p);
        CloseHandle(p);
        EXES.with(|m| m.borrow_mut().clear());
        result["note"] = json!("teardown.exe has no symbols: its frames are module+offset, with the enclosing function's start and, when server/names has a table for exactly this build, its name. Frames in Windows DLLs carry their exported name when one is near. A main thread sitting in a wait (NtWaitFor..., SleepConditionVariable...) is waiting for worker threads or the GPU; one deep in teardown.exe frames is computing.");
        Ok(result)
    }
}

// ---------------------------------------------------------------- decrypted code from memory

/// Save the game exe's image as it is in memory (its code decrypted by the game at start) as a
/// PE file whose sections sit at file offsets equal to their RVAs, for tools/port_names.py and
/// disassemblers. The file is the game's code: it stays on this machine.
pub fn save_code(g: &Game, out_dir: &Path) -> Result<Value, String> {
    unsafe {
        let p = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, g.pid);
        if p.is_null() {
            return Err(format!("cannot open the game process {} for reading", g.pid));
        }
        let result = save_code_from(p, g, out_dir);
        CloseHandle(p);
        result
    }
}

unsafe fn read_mem(p: HANDLE, addr: u64, buf: &mut [u8]) -> bool {
    let mut got = 0usize;
    ReadProcessMemory(p, addr as *const c_void, buf.as_mut_ptr() as *mut c_void, buf.len(), &mut got) != 0 && got == buf.len()
}

unsafe fn save_code_from(p: HANDLE, g: &Game, out_dir: &Path) -> Result<Value, String> {
    let mut module: *mut c_void = std::ptr::null_mut();
    let mut needed = 0u32;
    if K32EnumProcessModules(p, &mut module, std::mem::size_of::<*mut c_void>() as u32, &mut needed) == 0 || module.is_null() {
        return Err("cannot list the game's modules".into());
    }
    let base = module as u64;
    let mut head = vec![0u8; 0x1000];
    if !read_mem(p, base, &mut head) {
        return Err("cannot read the exe's headers in memory".into());
    }
    let r32 = |d: &[u8], o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let r16 = |d: &[u8], o: usize| u16::from_le_bytes(d[o..o + 2].try_into().unwrap()) as usize;
    let pe = r32(&head, 0x3c) as usize;
    let stamp = r32(&head, pe + 8);
    let nsec = r16(&head, pe + 6);
    let opt = pe + 24;
    let opt_size = r16(&head, pe + 20);
    let image_size = r32(&head, opt + 56) as usize;
    let section_align = r32(&head, opt + 32);

    // The whole image, page by page; a page that cannot be read stays zero.
    let mut img = vec![0u8; image_size];
    let mut unreadable = 0usize;
    for off in (0..image_size).step_by(0x1000) {
        let end = (off + 0x1000).min(image_size);
        if !read_mem(p, base + off as u64, &mut img[off..end]) {
            unreadable += 1;
        }
    }

    // Sections at file offset == RVA, so the file reads like the loaded image.
    let mut text = None;
    for i in 0..nsec {
        let s = opt + opt_size + i * 40;
        let vsize = r32(&img, s + 8);
        let va = r32(&img, s + 12);
        let name = String::from_utf8_lossy(&img[s..s + 8]).trim_end_matches('\0').to_string();
        if name == ".text" {
            text = Some((va, vsize, r32(&head, s + 20), r32(&head, s + 16)));
        }
        let raw = vsize.div_ceil(section_align) * section_align;
        img[s + 16..s + 20].copy_from_slice(&raw.min(image_size as u32 - va).to_le_bytes()); // SizeOfRawData
        img[s + 20..s + 24].copy_from_slice(&va.to_le_bytes()); // PointerToRawData
    }
    img[opt + 36..opt + 40].copy_from_slice(&section_align.to_le_bytes()); // FileAlignment
    img[opt + 24..opt + 32].copy_from_slice(&base.to_le_bytes()); // ImageBase: where it was loaded

    // Decrypted yet? Compare the start of .text with the file on disk.
    let mut decrypted = Value::Null;
    if let Some((va, vsize, rptr, rsize)) = text {
        let mut buf = [0u16; 520];
        let n = K32GetModuleFileNameExW(p, module, buf.as_mut_ptr(), buf.len() as u32);
        if let Ok(disk) = std::fs::read(String::from_utf16_lossy(&buf[..n as usize])) {
            let len = 0x10000.min(vsize as usize).min(rsize as usize);
            let a = &img[va as usize..va as usize + len];
            let b = disk.get(rptr as usize..rptr as usize + len).unwrap_or(&[]);
            let differ = a.iter().zip(b).filter(|(x, y)| x != y).count();
            decrypted = json!(differ > len / 4);
            if differ <= len / 4 {
                return Err("the code in memory still matches the encrypted file on disk: the game has not decrypted itself yet (just started?). Try again in a few seconds.".into());
            }
        }
    }

    let _ = std::fs::create_dir_all(out_dir);
    let stem = g.exe.trim_end_matches(".exe").to_lowercase();
    let path = out_dir.join(format!("{stem}-{stamp:08x}.exe"));
    std::fs::write(&path, &img).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(json!({
        "path": path, "exe": g.exe, "build_stamp": format!("{stamp:#010x}"),
        "size_mb": mb(img.len()), "loaded_at": format!("{base:#x}"),
        "unreadable_pages": unreadable, "decrypted": decrypted,
        "note": "The game's own code: keep it on this machine, never commit or share it. Use it with tools/port_names.py (--old / --new) or open it in Ghidra or IDA (it is laid out like the loaded image, ImageBase set to where it was loaded)."
    }))
}

// ---------------------------------------------------------------- health

pub fn health(log_dir: &Path, only: Option<&str>) -> Value {
    let gs = games(only);
    let before: Vec<Option<Counters>> = gs.iter().map(|g| counters(g.pid)).collect();
    std::thread::sleep(Duration::from_millis(500));
    let procs: Vec<Value> = gs
        .iter()
        .zip(before)
        .filter_map(|(g, a)| {
            let a = a?;
            let b = counters(g.pid)?;
            Some(process_json(g, &a, &b))
        })
        .collect();
    let ended = ended();
    json!({
        "game_windows": procs,
        "ended_since_last_check": ended,
        "recent_crash_reports": crash_list(log_dir, 5),
        "crash_report_count": crash_dirs(log_dir).len(),
        "gpu": gpu(),
        "note": "responding=false means Windows sees the window as hung (no messages handled for 5 s). Use crash_report for a report's details, dump_game for the stacks of a hung or slow game, ping to check the bridge itself."
    })
}
