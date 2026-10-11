use gpui::{Bounds, DisplayId, Pixels, Window, point, px, size};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::{
    ffi::{CString, c_void},
    path::Path,
};

unsafe extern "C" {
    fn lp_configure_overlay(view: *mut c_void, x: f64, y: f64, w: f64, h: f64);
    fn lp_play_sound(path: *const std::ffi::c_char, volume: f32) -> bool;
    fn lp_init_status();
    fn lp_status_enabled(enabled: bool);
    fn lp_poll_action() -> i32;
    fn lp_hide_window(view: *mut c_void);
    fn lp_display_bounds(display: u32, out: *mut f64);
    fn lp_overlay_valid(view: *mut c_void) -> bool;
}

fn view(window: &Window) -> *mut c_void {
    match HasWindowHandle::window_handle(window)
        .expect("macOS window handle")
        .as_raw()
    {
        RawWindowHandle::AppKit(handle) => handle.ns_view.as_ptr(),
        _ => unreachable!("macOS only"),
    }
}
pub fn configure_overlay(window: &Window, bounds: Bounds<Pixels>) {
    // The handle is borrowed only during this main-thread call; Objective-C does not retain it.
    unsafe {
        lp_configure_overlay(
            view(window),
            f32::from(bounds.origin.x) as f64,
            f32::from(bounds.origin.y) as f64,
            f32::from(bounds.size.width) as f64,
            f32::from(bounds.size.height) as f64,
        );
    }
}
pub fn hide(window: &Window) {
    unsafe {
        lp_hide_window(view(window));
    }
}
pub fn play(path: &Path, volume: u32) -> bool {
    let Ok(path) = CString::new(path.to_string_lossy().as_bytes()) else {
        return false;
    };
    unsafe { lp_play_sound(path.as_ptr(), volume as f32 / 100.) }
}
pub fn init_status() {
    unsafe {
        lp_init_status();
    }
}
pub fn set_enabled(enabled: bool) {
    unsafe {
        lp_status_enabled(enabled);
    }
}
pub fn poll_action() -> i32 {
    unsafe { lp_poll_action() }
}

pub fn overlay_valid(window: &Window) -> bool {
    unsafe { lp_overlay_valid(view(window)) }
}

/// GPUI 0.2.2 returns a zero origin for every Mac display. Read global points from CoreGraphics.
pub fn display_bounds(id: DisplayId) -> Bounds<Pixels> {
    let mut values = [0.0_f64; 4];
    unsafe {
        lp_display_bounds(id.into(), values.as_mut_ptr());
    }
    Bounds {
        origin: point(px(values[0] as f32), px(values[1] as f32)),
        size: size(px(values[2] as f32), px(values[3] as f32)),
    }
}
