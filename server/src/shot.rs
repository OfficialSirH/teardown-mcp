//! Screenshot of the Teardown window (Windows GDI), returned as a JPEG.

#![allow(non_snake_case, clippy::upper_case_acronyms)]

use std::ffi::c_void;

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
#[derive(Default)]
struct BITMAPINFOHEADER {
    biSize: u32,
    biWidth: i32,
    biHeight: i32,
    biPlanes: u16,
    biBitCount: u16,
    biCompression: u32,
    biSizeImage: u32,
    biXPelsPerMeter: i32,
    biYPelsPerMeter: i32,
    biClrUsed: u32,
    biClrImportant: u32,
}

#[repr(C)]
#[derive(Default)]
struct BITMAPINFO {
    header: BITMAPINFOHEADER,
    colors: [u32; 3],
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> HANDLE;
    fn GetClientRect(hwnd: HANDLE, rect: *mut RECT) -> i32;
    fn ClientToScreen(hwnd: HANDLE, pt: *mut POINT) -> i32;
    fn IsIconic(hwnd: HANDLE) -> i32;
    fn GetDC(hwnd: HANDLE) -> HANDLE;
    fn ReleaseDC(hwnd: HANDLE, dc: HANDLE) -> i32;
    fn PrintWindow(hwnd: HANDLE, dc: HANDLE, flags: u32) -> i32;
    fn SetProcessDpiAwarenessContext(ctx: isize) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(dc: HANDLE) -> HANDLE;
    fn CreateCompatibleBitmap(dc: HANDLE, w: i32, h: i32) -> HANDLE;
    fn SelectObject(dc: HANDLE, obj: HANDLE) -> HANDLE;
    fn BitBlt(dst: HANDLE, x: i32, y: i32, w: i32, h: i32, src: HANDLE, sx: i32, sy: i32, rop: u32) -> i32;
    fn GetDIBits(dc: HANDLE, bmp: HANDLE, start: u32, lines: u32, bits: *mut c_void, info: *mut BITMAPINFO, usage: u32) -> i32;
    fn DeleteObject(obj: HANDLE) -> i32;
    fn DeleteDC(dc: HANDLE) -> i32;
}

/// Grab the window's client area as BGRA. `from_screen` copies what is on the monitor
/// (the window must be visible); otherwise the window is asked to draw itself.
unsafe fn grab(hwnd: HANDLE, w: i32, h: i32, from_screen: bool) -> Option<Vec<u8>> {
    let screen = GetDC(std::ptr::null_mut());
    let mem = CreateCompatibleDC(screen);
    let bmp = CreateCompatibleBitmap(screen, w, h);
    let old = SelectObject(mem, bmp);
    let ok = if from_screen {
        let mut pt = POINT::default();
        ClientToScreen(hwnd, &mut pt);
        BitBlt(mem, 0, 0, w, h, screen, pt.x, pt.y, 0x00CC0020) != 0
    } else {
        // PW_CLIENTONLY | PW_RENDERFULLCONTENT
        PrintWindow(hwnd, mem, 3) != 0
    };
    SelectObject(mem, old);
    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    let mut info = BITMAPINFO::default();
    info.header.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
    info.header.biWidth = w;
    info.header.biHeight = -h; // top-down
    info.header.biPlanes = 1;
    info.header.biBitCount = 32;
    let lines = GetDIBits(screen, bmp, 0, h as u32, buf.as_mut_ptr() as *mut c_void, &mut info, 0);
    DeleteObject(bmp);
    DeleteDC(mem);
    ReleaseDC(std::ptr::null_mut(), screen);
    if ok && lines == h {
        Some(buf)
    } else {
        None
    }
}

fn is_black(bgra: &[u8]) -> bool {
    bgra.chunks_exact(4).step_by(97).all(|p| p[0] < 8 && p[1] < 8 && p[2] < 8)
}

/// Is there a top-level window with exactly this title?
pub fn window_exists(title: &str) -> bool {
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe { !FindWindowW(std::ptr::null(), wide.as_ptr()).is_null() }
}

pub struct Shot {
    pub jpeg: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub method: &'static str,
}

/// Window titles tried when none is given: the multiplayer test's host window, then the game.
const DEFAULT_TITLES: [&str; 2] = ["TD - Host", "Teardown"];

pub fn capture(max_width: u32, window: Option<&str>) -> Result<Shot, String> {
    unsafe {
        // Real pixels, not DPI-scaled ones (-4 = per-monitor aware v2).
        SetProcessDpiAwarenessContext(-4);
        let titles: Vec<&str> = match window {
            Some(t) => vec![t],
            None => DEFAULT_TITLES.to_vec(),
        };
        let mut hwnd: HANDLE = std::ptr::null_mut();
        for t in &titles {
            let wide: Vec<u16> = t.encode_utf16().chain(std::iter::once(0)).collect();
            hwnd = FindWindowW(std::ptr::null(), wide.as_ptr());
            if !hwnd.is_null() {
                break;
            }
        }
        if hwnd.is_null() {
            return Err(format!("no window titled {titles:?} found; is the game running?"));
        }
        if IsIconic(hwnd) != 0 {
            return Err("the Teardown window is minimized".into());
        }
        let mut rc = RECT::default();
        GetClientRect(hwnd, &mut rc);
        let (w, h) = (rc.right - rc.left, rc.bottom - rc.top);
        if w <= 0 || h <= 0 {
            return Err("the Teardown window has no visible area".into());
        }
        let mut method = "window";
        let mut px = grab(hwnd, w, h, false).filter(|b| !is_black(b));
        if px.is_none() {
            method = "screen (window must not be covered)";
            px = grab(hwnd, w, h, true);
        }
        let px = px.ok_or("capturing the window failed")?;

        let mut rgb = Vec::with_capacity((w * h * 3) as usize);
        for p in px.chunks_exact(4) {
            rgb.extend_from_slice(&[p[2], p[1], p[0]]);
        }
        let mut img = image::RgbImage::from_raw(w as u32, h as u32, rgb).ok_or("bad image buffer")?;
        if img.width() > max_width {
            let nh = (img.height() as u64 * max_width as u64 / img.width() as u64).max(1) as u32;
            img = image::imageops::resize(&img, max_width, nh, image::imageops::FilterType::Triangle);
        }
        let mut jpeg = Vec::new();
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, 82)
            .encode_image(&img)
            .map_err(|e| e.to_string())?;
        Ok(Shot { jpeg, width: img.width(), height: img.height(), method })
    }
}
