//! Keyboard and mouse input for a game window (Windows `SendInput`).
//!
//! The window is brought to the front, events are sent only while it really is the foreground
//! window, and the previous foreground window gets focus back afterwards.

#![allow(non_snake_case)]

use serde_json::Value;
use std::ffi::c_void;
use std::time::Duration;

type HANDLE = *mut c_void;

#[repr(C)]
#[derive(Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra: usize,
}

#[repr(C)]
struct KeybdInput {
    vk: u16,
    scan: u16,
    flags: u32,
    time: u32,
    extra: usize,
}

// INPUT is a tagged union of 40 bytes on x64; these are its two layouts used here.
#[repr(C)]
struct InputMouse {
    ty: u32,
    mi: MouseInput,
}

#[repr(C)]
struct InputKey {
    ty: u32,
    ki: KeybdInput,
    pad: [u8; 8],
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> HANDLE;
    fn GetForegroundWindow() -> HANDLE;
    fn SetForegroundWindow(hwnd: HANDLE) -> i32;
    fn IsIconic(hwnd: HANDLE) -> i32;
    fn GetClientRect(hwnd: HANDLE, rect: *mut RECT) -> i32;
    fn ClientToScreen(hwnd: HANDLE, pt: *mut POINT) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn SendInput(count: u32, inputs: *const c_void, size: i32) -> u32;
    fn MapVirtualKeyW(code: u32, map_type: u32) -> u32;
    fn SetProcessDpiAwarenessContext(ctx: isize) -> i32;
}

const KEYEVENTF_EXTENDEDKEY: u32 = 0x1;
const KEYEVENTF_KEYUP: u32 = 0x2;
const KEYEVENTF_SCANCODE: u32 = 0x8;

/// Key name -> (virtual key, extended).
fn vk(name: &str) -> Option<(u32, bool)> {
    let n = name.to_lowercase();
    let b = n.as_bytes();
    if b.len() == 1 && (b[0].is_ascii_lowercase() || b[0].is_ascii_digit()) {
        return Some((b[0].to_ascii_uppercase() as u32, false));
    }
    if let Some(num) = n.strip_prefix('f').and_then(|x| x.parse::<u32>().ok()) {
        if (1..=12).contains(&num) {
            return Some((0x70 + num - 1, false));
        }
    }
    Some(match n.as_str() {
        "space" => (0x20, false),
        "enter" | "return" => (0x0D, false),
        "esc" | "escape" => (0x1B, false),
        "tab" => (0x09, false),
        "backspace" => (0x08, false),
        "shift" => (0xA0, false),
        "ctrl" | "control" => (0xA2, false),
        "alt" => (0xA4, false),
        "up" => (0x26, true),
        "down" => (0x28, true),
        "left" => (0x25, true),
        "right" => (0x27, true),
        "delete" => (0x2E, true),
        "home" => (0x24, true),
        "end" => (0x23, true),
        "pageup" => (0x21, true),
        "pagedown" => (0x22, true),
        _ => return None,
    })
}

unsafe fn key(code: u32, extended: bool, up: bool) {
    let scan = MapVirtualKeyW(code, 0) as u16;
    let mut flags = KEYEVENTF_SCANCODE;
    if extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    let input = InputKey { ty: 1, ki: KeybdInput { vk: 0, scan, flags, time: 0, extra: 0 }, pad: [0; 8] };
    SendInput(1, &input as *const _ as *const c_void, std::mem::size_of::<InputKey>() as i32);
}

unsafe fn mouse(flags: u32, dx: i32, dy: i32, data: u32) {
    let input = InputMouse { ty: 0, mi: MouseInput { dx, dy, mouse_data: data, flags, time: 0, extra: 0 } };
    SendInput(1, &input as *const _ as *const c_void, std::mem::size_of::<InputMouse>() as i32);
}

fn find(title: &str) -> HANDLE {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { FindWindowW(std::ptr::null(), wide.as_ptr()) }
}

/// Bring `hwnd` to the front. Windows refuses a plain SetForegroundWindow from a background
/// process; a tap of Alt lifts that restriction.
unsafe fn focus(hwnd: HANDLE) -> bool {
    for _ in 0..4 {
        if GetForegroundWindow() == hwnd {
            return true;
        }
        key(0xA4, false, false);
        key(0xA4, false, true);
        SetForegroundWindow(hwnd);
        std::thread::sleep(Duration::from_millis(120));
    }
    GetForegroundWindow() == hwnd
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

/// Run the actions. Returns a short report of what was sent.
pub fn send(actions: &[Value], window: Option<&str>, restore: bool) -> Result<String, String> {
    if actions.is_empty() {
        return Err("no actions".into());
    }
    if actions.len() > 60 {
        return Err("at most 60 actions per call".into());
    }
    unsafe {
        SetProcessDpiAwarenessContext(-4);
        let titles: Vec<&str> = match window {
            Some(t) => vec![t],
            None => vec!["TD - Host", "Teardown"],
        };
        let hwnd = titles.iter().map(|t| find(t)).find(|h| !h.is_null()).unwrap_or(std::ptr::null_mut());
        if hwnd.is_null() {
            return Err(format!("no window titled {titles:?} found"));
        }
        if IsIconic(hwnd) != 0 {
            return Err("the game window is minimized".into());
        }
        let previous = GetForegroundWindow();
        if !focus(hwnd) {
            return Err("could not bring the game window to the front; no input was sent".into());
        }
        sleep_ms(150);

        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc);
        let mut origin = POINT::default();
        ClientToScreen(hwnd, &mut origin);

        let mut done = Vec::new();
        let mut failure = None;
        for a in actions {
            // Never type into another window: stop the moment the game loses focus.
            if GetForegroundWindow() != hwnd {
                failure = Some("the game window lost focus; stopped".to_string());
                break;
            }
            let hold = a["hold_ms"].as_u64().unwrap_or(60).clamp(10, 5000);
            if let Some(name) = a["key"].as_str() {
                // "shift+alt+f3": press in order, release in reverse.
                let codes: Option<Vec<(u32, bool)>> = name.split('+').map(|k| vk(k.trim())).collect();
                let Some(codes) = codes else {
                    failure = Some(format!("unknown key in \"{name}\""));
                    break;
                };
                for &(code, ext) in &codes {
                    key(code, ext, false);
                    sleep_ms(20);
                }
                sleep_ms(hold);
                for &(code, ext) in codes.iter().rev() {
                    key(code, ext, true);
                    sleep_ms(20);
                }
                done.push(format!("key {name} ({hold} ms)"));
            } else if let Some(button) = a["click"].as_str() {
                if let (Some(x), Some(y)) = (a["x"].as_f64(), a["y"].as_f64()) {
                    let px = origin.x + (x.clamp(0.0, 1.0) * (rc.right - rc.left) as f64) as i32;
                    let py = origin.y + (y.clamp(0.0, 1.0) * (rc.bottom - rc.top) as f64) as i32;
                    SetCursorPos(px, py);
                    // A small real movement so the game notices the new cursor position.
                    mouse(0x0001, 1, 0, 0);
                    mouse(0x0001, -1, 0, 0);
                    sleep_ms(80);
                }
                let (down, up) = match button {
                    "right" => (0x0008, 0x0010),
                    "middle" => (0x0020, 0x0040),
                    _ => (0x0002, 0x0004),
                };
                mouse(down, 0, 0, 0);
                sleep_ms(hold);
                mouse(up, 0, 0, 0);
                done.push(format!("{button} click ({hold} ms)"));
            } else if let (Some(from), Some(to)) = (a["drag"].as_array(), a["to"].as_array()) {
                let at = |v: &Vec<Value>| -> (i32, i32) {
                    let x = v.first().and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0);
                    let y = v.get(1).and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0);
                    (
                        origin.x + (x * (rc.right - rc.left) as f64) as i32,
                        origin.y + (y * (rc.bottom - rc.top) as f64) as i32,
                    )
                };
                let (x0, y0) = at(from);
                let (x1, y1) = at(to);
                SetCursorPos(x0, y0);
                mouse(0x0001, 1, 0, 0);
                mouse(0x0001, -1, 0, 0);
                sleep_ms(80);
                mouse(0x0002, 0, 0, 0);
                sleep_ms(60);
                let steps = 12;
                for i in 1..=steps {
                    SetCursorPos(x0 + (x1 - x0) * i / steps, y0 + (y1 - y0) * i / steps);
                    mouse(0x0001, 1, 0, 0);
                    mouse(0x0001, -1, 0, 0);
                    sleep_ms(hold / steps as u64 + 10);
                }
                sleep_ms(60);
                mouse(0x0004, 0, 0, 0);
                done.push("drag".to_string());
            } else if let Some(m) = a["move"].as_array() {
                let dx = m.first().and_then(Value::as_i64).unwrap_or(0) as i32;
                let dy = m.get(1).and_then(Value::as_i64).unwrap_or(0) as i32;
                // In steps: one huge jump is often clamped or ignored.
                let steps = ((dx.abs().max(dy.abs()) / 40) + 1).min(100);
                for i in 1..=steps {
                    mouse(0x0001, dx * i / steps - dx * (i - 1) / steps, dy * i / steps - dy * (i - 1) / steps, 0);
                    sleep_ms(8);
                }
                done.push(format!("mouse move {dx},{dy}"));
            } else if let Some(n) = a["scroll"].as_i64() {
                mouse(0x0800, 0, 0, (n as i32 * 120) as u32);
                done.push(format!("scroll {n}"));
            } else if let Some(ms) = a["wait_ms"].as_u64() {
                sleep_ms(ms.min(5000));
                done.push(format!("wait {ms} ms"));
            } else {
                failure = Some(format!("unknown action {a}"));
                break;
            }
            sleep_ms(40);
        }
        if restore && !previous.is_null() && previous != hwnd {
            sleep_ms(60);
            key(0xA4, false, false);
            key(0xA4, false, true);
            SetForegroundWindow(previous);
        }
        match failure {
            Some(f) => Err(format!("{f}. Sent before that: {}", done.join(", "))),
            None => Ok(done.join(", ")),
        }
    }
}
