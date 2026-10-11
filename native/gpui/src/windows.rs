use gpui::{Bounds, DisplayId, Pixels, Window, point, px, size};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{ffi::c_void, os::windows::ffi::OsStrExt, path::Path};

pub const FONT: &str = "Segoe UI";
pub const QUIT_KEY: &str = "ctrl-q";
pub const SETTINGS_KEY: &str = "ctrl-,";
pub const UNDO_KEY: &str = "ctrl-z";

unsafe extern "C" {
    fn lp_configure_settings(hwnd: *mut c_void);
    fn lp_configure_overlay(hwnd: *mut c_void, x: f64, y: f64, w: f64, h: f64);
    fn lp_play_sound(path: *const u16, volume: f32) -> bool;
    fn lp_init_status() -> bool;
    fn lp_shutdown();
    fn lp_status_enabled(enabled: bool);
    fn lp_poll_action() -> i32;
    fn lp_hide_window(hwnd: *mut c_void);
    fn lp_show_window(hwnd: *mut c_void);
    fn lp_display_bounds(display: u32, out: *mut f64);
    fn lp_overlay_valid(hwnd: *mut c_void) -> bool;
}

fn handle(window: &Window) -> *mut c_void {
    match window
        .window_handle()
        .expect("Windows window handle")
        .as_raw()
    {
        RawWindowHandle::Win32(handle) => handle.hwnd.get() as *mut c_void,
        _ => unreachable!("Windows only"),
    }
}

pub fn configure_settings(window: &Window) {
    unsafe { lp_configure_settings(handle(window)) }
}

pub fn configure_overlay(window: &Window, bounds: Bounds<Pixels>) {
    unsafe {
        lp_configure_overlay(
            handle(window),
            f32::from(bounds.origin.x) as f64,
            f32::from(bounds.origin.y) as f64,
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        );
    }
}

pub fn hide(window: &Window) {
    unsafe { lp_hide_window(handle(window)) }
}

pub fn show(window: &Window) {
    unsafe { lp_show_window(handle(window)) }
}

pub fn play(path: &Path, volume: u32) -> bool {
    let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe { lp_play_sound(path.as_ptr(), volume as f32 / 100.) }
}

pub fn init_status() -> bool {
    unsafe { lp_init_status() }
}

pub fn shutdown() {
    unsafe { lp_shutdown() }
}

pub fn set_enabled(enabled: bool) {
    unsafe { lp_status_enabled(enabled) }
}

pub fn poll_action() -> i32 {
    unsafe { lp_poll_action() }
}

pub fn overlay_valid(window: &Window) -> bool {
    unsafe { lp_overlay_valid(handle(window)) }
}

// Keep global input/display coordinates in physical pixels. Converting each display's
// global origin to logical pixels creates overlapping rectangles on mixed-DPI desktops.
fn display_values(id: DisplayId) -> [f64; 5] {
    let mut values = [0., 0., 0., 0., 1.];
    unsafe { lp_display_bounds(id.into(), values.as_mut_ptr()) }
    values
}

pub fn display_bounds(id: DisplayId) -> Bounds<Pixels> {
    let v = display_values(id);
    Bounds::new(
        point(px(v[0] as f32), px(v[1] as f32)),
        size(px(v[2] as f32), px(v[3] as f32)),
    )
}

pub fn display_scale(id: DisplayId) -> f32 {
    display_values(id)[4] as f32
}
