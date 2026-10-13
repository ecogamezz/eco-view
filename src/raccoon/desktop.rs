//! macOS glue: the global cursor and mouse button, other apps' windows, the
//! displays, and the overlay window's AppKit settings.
//!
//! Everything here is in global desktop points with a top-left origin on the
//! primary display, which is what CoreGraphics and winit both use. AppKit's
//! screen frames are bottom-left based and get converted.

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui::{Pos2, Rect, pos2, vec2};

use super::WinRect;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGSize {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGRect {
    origin: CGPoint,
    size: CGSize,
}

type CFTypeRef = *const c_void;

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreate(source: CFTypeRef) -> CFTypeRef;
    fn CGEventGetLocation(event: CFTypeRef) -> CGPoint;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
    fn CGEventSourceCounterForEventType(state: i32, event_type: u32) -> u32;
    fn CGWindowListCopyWindowInfo(option: u32, relative_to: u32) -> CFTypeRef;
    fn CGRectMakeWithDictionaryRepresentation(dict: CFTypeRef, rect: *mut CGRect) -> bool;
    static kCGWindowNumber: CFTypeRef;
    static kCGWindowLayer: CFTypeRef;
    static kCGWindowBounds: CFTypeRef;
    static kCGWindowOwnerPID: CFTypeRef;
    static kCGWindowAlpha: CFTypeRef;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(cf: CFTypeRef);
    fn CFArrayGetCount(array: CFTypeRef) -> isize;
    fn CFArrayGetValueAtIndex(array: CFTypeRef, index: isize) -> CFTypeRef;
    fn CFDictionaryGetValue(dict: CFTypeRef, key: CFTypeRef) -> CFTypeRef;
    fn CFNumberGetValue(number: CFTypeRef, kind: isize, out: *mut c_void) -> bool;
}

const COMBINED_SESSION: i32 = 0;
const LEFT_BUTTON: u32 = 0;
const LEFT_MOUSE_DOWN: u32 = 1;
const ON_SCREEN_ONLY: u32 = 1 << 0;
const EXCLUDE_DESKTOP: u32 = 1 << 4;
const CF_SINT64: isize = 4;
const CF_DOUBLE: isize = 13;

/// The pointer as the whole desktop sees it, independent of which window (if
/// any) is receiving events.
#[derive(Default)]
pub struct Mouse {
    last_count: Option<u32>,
}

#[derive(Clone, Copy, Debug)]
pub struct MouseState {
    pub pos: Pos2,
    pub down: bool,
    pub presses: u32,
}

impl Mouse {
    pub fn poll(&mut self) -> Option<MouseState> {
        // SAFETY: plain CoreGraphics calls; the event is released below.
        unsafe {
            let ev = CGEventCreate(std::ptr::null());
            if ev.is_null() {
                return None;
            }
            let p = CGEventGetLocation(ev);
            CFRelease(ev);
            let down = CGEventSourceButtonState(COMBINED_SESSION, LEFT_BUTTON);
            let count = CGEventSourceCounterForEventType(COMBINED_SESSION, LEFT_MOUSE_DOWN);
            let presses = match self.last_count {
                Some(last) => count.wrapping_sub(last).min(8),
                None => 0,
            };
            self.last_count = Some(count);
            Some(MouseState { pos: pos2(p.x as f32, p.y as f32), down, presses })
        }
    }
}

unsafe fn dict_i64(dict: CFTypeRef, key: CFTypeRef) -> Option<i64> {
    // SAFETY: caller passes a valid dictionary; the value is borrowed.
    unsafe {
        let v = CFDictionaryGetValue(dict, key);
        if v.is_null() {
            return None;
        }
        let mut out: i64 = 0;
        CFNumberGetValue(v, CF_SINT64, &mut out as *mut i64 as *mut c_void).then_some(out)
    }
}

unsafe fn dict_f64(dict: CFTypeRef, key: CFTypeRef) -> Option<f64> {
    // SAFETY: as above.
    unsafe {
        let v = CFDictionaryGetValue(dict, key);
        if v.is_null() {
            return None;
        }
        let mut out: f64 = 0.0;
        CFNumberGetValue(v, CF_DOUBLE, &mut out as *mut f64 as *mut c_void).then_some(out)
    }
}

/// Lists ordinary on-screen windows, front to back. Window bounds are
/// available without the Screen Recording permission (titles are not, and
/// aren't needed).
pub fn list_windows() -> Vec<WinRect> {
    let own = std::process::id() as i64;
    let mut out = Vec::new();
    // SAFETY: the array is owned by us (Copy rule) and released at the end;
    // dictionaries and numbers inside are borrowed from it.
    unsafe {
        let list = CGWindowListCopyWindowInfo(ON_SCREEN_ONLY | EXCLUDE_DESKTOP, 0);
        if list.is_null() {
            return out;
        }
        for i in 0..CFArrayGetCount(list) {
            let d = CFArrayGetValueAtIndex(list, i);
            if d.is_null() {
                continue;
            }
            if dict_i64(d, kCGWindowLayer) != Some(0) {
                continue;
            }
            if dict_f64(d, kCGWindowAlpha).unwrap_or(1.0) < 0.05 {
                continue;
            }
            let bounds = CFDictionaryGetValue(d, kCGWindowBounds);
            let mut r = CGRect::default();
            if bounds.is_null() || !CGRectMakeWithDictionaryRepresentation(bounds, &mut r) {
                continue;
            }
            if r.size.width < 80.0 || r.size.height < 50.0 {
                continue;
            }
            let id = dict_i64(d, kCGWindowNumber).unwrap_or(0) as u32;
            let pid = dict_i64(d, kCGWindowOwnerPID).unwrap_or(-1);
            out.push(WinRect {
                id,
                rect: Rect::from_min_size(
                    pos2(r.origin.x as f32, r.origin.y as f32),
                    vec2(r.size.width as f32, r.size.height as f32),
                ),
                own: pid == own,
            });
        }
        CFRelease(list);
    }
    out
}

/// Polls the window list on a background thread so the render loop never
/// waits on the window server. Idles while the raccoon is put away.
#[derive(Clone)]
pub struct WindowWatcher {
    shared: Arc<Mutex<(u64, Vec<WinRect>)>>,
    active: Arc<AtomicBool>,
}

impl WindowWatcher {
    pub fn spawn() -> Self {
        let shared = Arc::new(Mutex::new((0u64, Vec::new())));
        let active = Arc::new(AtomicBool::new(false));
        let (s, a) = (shared.clone(), active.clone());
        std::thread::Builder::new()
            .name("eco-raccoon-windows".into())
            .spawn(move || {
                let mut last = Vec::new();
                loop {
                    if !a.load(Ordering::Relaxed) {
                        std::thread::sleep(Duration::from_millis(400));
                        continue;
                    }
                    let now = list_windows();
                    if now != last {
                        if let Ok(mut g) = s.lock() {
                            g.0 += 1;
                            g.1 = now.clone();
                        }
                        last = now;
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            })
            .ok();
        Self { shared, active }
    }

    pub fn set_active(&self, on: bool) {
        self.active.store(on, Ordering::Relaxed);
    }

    /// Returns the list if it changed since `seen`.
    pub fn take_if_newer(&self, seen: &mut u64) -> Option<Vec<WinRect>> {
        let g = self.shared.lock().ok()?;
        if g.0 != *seen {
            *seen = g.0;
            Some(g.1.clone())
        } else {
            None
        }
    }
}

/// A display: its full frame and the part not covered by the menu bar and Dock.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Display {
    pub frame: Rect,
    pub visible: Rect,
}

/// All displays, in top-left global coordinates. Must run on the main thread.
pub fn displays() -> Vec<Display> {
    use objc2_app_kit::NSScreen;
    use objc2_foundation::MainThreadMarker;

    let Some(mtm) = MainThreadMarker::new() else { return Vec::new() };
    let screens = NSScreen::screens(mtm);
    let mut out = Vec::new();
    let mut primary_h = None;
    for s in screens.iter() {
        let f = s.frame();
        let v = s.visibleFrame();
        let ph = *primary_h.get_or_insert(f.size.height);
        let conv = |r: objc2_foundation::NSRect| {
            Rect::from_min_size(
                pos2(r.origin.x as f32, (ph - r.origin.y - r.size.height) as f32),
                vec2(r.size.width as f32, r.size.height as f32),
            )
        };
        out.push(Display { frame: conv(f), visible: conv(v) });
    }
    out
}

/// The display containing `p`, else the primary one.
pub fn display_at(p: Pos2) -> Option<Display> {
    let all = displays();
    all.iter().find(|d| d.frame.contains(p)).copied().or_else(|| all.first().copied())
}

/// Makes the overlay behave like a desktop pet: visible on every Space and
/// over full-screen apps, no window shadow, out of the window cycle.
/// Returns true once the window has been found and configured.
///
/// AppKit nudges new windows clear of the menu bar, so the frame is set here
/// directly to cover the whole display.
pub fn configure_overlay(title: &str, stage: Rect) -> bool {
    use objc2_app_kit::{NSApplication, NSScreen, NSWindowCollectionBehavior};
    use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize};

    let Some(mtm) = MainThreadMarker::new() else { return false };
    let primary_h = NSScreen::screens(mtm).iter().next().map(|s| s.frame().size.height).unwrap_or(0.0);
    let frame = NSRect::new(
        NSPoint::new(stage.min.x as f64, primary_h - stage.max.y as f64),
        NSSize::new(stage.width() as f64, stage.height() as f64),
    );
    let app = NSApplication::sharedApplication(mtm);
    let windows = app.windows();
    let mut done = false;
    for w in windows.iter() {
        if w.title().to_string() != title {
            continue;
        }
        w.setHasShadow(false);
        if primary_h > 0.0 {
            w.setFrame_display(frame, true);
        }
        // SAFETY: plain property setter on a live window, on the main thread.
        unsafe {
            w.setCollectionBehavior(
                NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::Stationary
                    | NSWindowCollectionBehavior::IgnoresCycle
                    | NSWindowCollectionBehavior::FullScreenAuxiliary,
            );
        }
        done = true;
    }
    done
}

/// The app that was frontmost before the user clicked the raccoon, so focus
/// can be handed back afterwards instead of leaving eco view in front.
pub struct Focus {
    pid: Option<i32>,
}

impl Focus {
    pub fn capture() -> Self {
        use objc2_app_kit::NSWorkspace;
        // SAFETY: NSWorkspace is thread-safe for these reads.
        let pid = unsafe {
            NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .map(|a| a.processIdentifier())
        };
        let own = std::process::id() as i32;
        Self { pid: pid.filter(|p| *p != own) }
    }

    pub fn restore(&self) {
        use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
        let Some(pid) = self.pid else { return };
        // SAFETY: activating another app by pid; harmless if it has quit.
        unsafe {
            if let Some(app) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) {
                app.activateWithOptions(NSApplicationActivationOptions::empty());
            }
        }
    }
}
