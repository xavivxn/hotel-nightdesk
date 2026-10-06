//! Keeps the main window inside the visible area of its monitor.
//!
//! On Windows the window has no native frame: minimize, maximize and close live in the web
//! title bar. If the window is larger than the monitor's work area (screen minus taskbar), or
//! is centered on a smaller screen, its top edge ends up above the screen and those buttons
//! become unreachable. Size and position are therefore derived from the work area and the
//! display scale of the monitor, never from fixed pixels.

/// Size the layout was designed for, in logical pixels.
const PREFERRED: (f64, f64) = (1680.0, 1050.0);
/// Smallest size the desktop layout is meant for; reduced further on smaller work areas.
const MINIMUM: (f64, f64) = (1024.0, 600.0);
/// Space kept around the window when it opens without maximizing.
const MARGIN: f64 = 24.0;

/// Rectangle in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement {
    /// Outer rectangle used as the normal (restored) window.
    pub rect: Rect,
    /// Minimum outer size, never larger than the work area.
    pub min: (u32, u32),
    /// Open maximized when the preferred size does not fit with its margin.
    pub maximize: bool,
}

fn to_physical(logical: f64, scale: f64) -> u32 {
    (logical * scale).round().max(1.0) as u32
}

pub(crate) fn minimum_for(work: Rect, scale: f64) -> (u32, u32) {
    (
        to_physical(MINIMUM.0, scale).min(work.width),
        to_physical(MINIMUM.1, scale).min(work.height),
    )
}

/// Initial placement inside the monitor work area `work` for a display `scale`.
pub(crate) fn initial_placement(work: Rect, scale: f64) -> Placement {
    let min = minimum_for(work, scale);
    let margin = to_physical(MARGIN, scale);
    let (preferred_w, preferred_h) = (
        to_physical(PREFERRED.0, scale),
        to_physical(PREFERRED.1, scale),
    );
    let fits = preferred_w + 2 * margin <= work.width && preferred_h + 2 * margin <= work.height;
    let width = preferred_w
        .min(work.width.saturating_sub(2 * margin))
        .max(min.0)
        .min(work.width);
    let height = preferred_h
        .min(work.height.saturating_sub(2 * margin))
        .max(min.1)
        .min(work.height);
    Placement {
        rect: Rect {
            x: work.x + ((work.width - width) / 2) as i32,
            y: work.y + ((work.height - height) / 2) as i32,
            width,
            height,
        },
        min,
        maximize: !fits,
    }
}

/// Shrinks and moves `window` so it lies completely inside `work`.
/// Returns `None` when it already does.
pub(crate) fn clamp_into(window: Rect, work: Rect) -> Option<Rect> {
    let width = window.width.min(work.width);
    let height = window.height.min(work.height);
    let max_x = work.x + (work.width - width) as i32;
    let max_y = work.y + (work.height - height) as i32;
    let next = Rect {
        x: window.x.clamp(work.x, max_x),
        y: window.y.clamp(work.y, max_y),
        width,
        height,
    };
    (next != window).then_some(next)
}

use tauri::{window::Monitor, PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

fn monitor_of<R: Runtime>(window: &WebviewWindow<R>) -> Option<Monitor> {
    window
        .current_monitor()
        .ok()
        .flatten()
        .or_else(|| window.primary_monitor().ok().flatten())
}

fn work_area(monitor: &Monitor) -> Rect {
    let area = monitor.work_area();
    Rect {
        x: area.position.x,
        y: area.position.y,
        width: area.size.width,
        height: area.size.height,
    }
}

/// Outer size minus inner size: the frame added by the OS, if any.
fn frame<R: Runtime>(window: &WebviewWindow<R>) -> (u32, u32) {
    match (window.outer_size(), window.inner_size()) {
        (Ok(outer), Ok(inner)) => (
            outer.width.saturating_sub(inner.width),
            outer.height.saturating_sub(inner.height),
        ),
        _ => (0, 0),
    }
}

fn apply_rect<R: Runtime>(window: &WebviewWindow<R>, rect: Rect) {
    let (frame_w, frame_h) = frame(window);
    let _ = window.set_size(PhysicalSize::new(
        rect.width.saturating_sub(frame_w).max(1),
        rect.height.saturating_sub(frame_h).max(1),
    ));
    let _ = window.set_position(PhysicalPosition::new(rect.x, rect.y));
}

fn apply_minimum<R: Runtime>(window: &WebviewWindow<R>, min: (u32, u32)) {
    let (frame_w, frame_h) = frame(window);
    let _ = window.set_min_size(Some(PhysicalSize::new(
        min.0.saturating_sub(frame_w).max(1),
        min.1.saturating_sub(frame_h).max(1),
    )));
}

/// Startup: fit the window to the monitor it opens on and maximize on small screens.
pub fn place_initial<R: Runtime>(window: &WebviewWindow<R>) {
    let _ = window.set_fullscreen(false);
    let _ = window.unmaximize();
    let Some(monitor) = monitor_of(window) else { return };
    let placement = initial_placement(work_area(&monitor), monitor.scale_factor());
    apply_minimum(window, placement.min);
    apply_rect(window, placement.rect);
    if placement.maximize {
        let _ = window.maximize();
    }
}

/// Re-showing from the tray or a second launch: bring the window back inside the work area
/// (monitor unplugged, resolution or scale changed while it was hidden).
pub fn ensure_visible<R: Runtime>(window: &WebviewWindow<R>) {
    let Some(monitor) = monitor_of(window) else { return };
    let work = work_area(&monitor);
    apply_minimum(window, minimum_for(work, monitor.scale_factor()));
    if window.is_maximized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        return;
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let current = Rect {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    };
    if let Some(next) = clamp_into(current, work) {
        apply_rect(window, next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn work(width: u32, height: u32) -> Rect {
        Rect { x: 0, y: 0, width, height }
    }

    fn inside(rect: Rect, area: Rect) -> bool {
        rect.x >= area.x
            && rect.y >= area.y
            && rect.x + rect.width as i32 <= area.x + area.width as i32
            && rect.y + rect.height as i32 <= area.y + area.height as i32
    }

    #[test]
    fn common_monitors_always_fit() {
        // (physical work area, scale): laptops and desktops seen in receptions.
        let cases = [
            (work(1366, 728), 1.0),
            (work(1366, 728), 1.25),
            (work(1280, 680), 1.0),
            (work(1600, 860), 1.25),
            (work(1920, 1040), 1.0),
            (work(1920, 1032), 1.25),
            (work(1920, 1020), 1.5),
            (work(2560, 1392), 1.0),
            (work(2560, 1380), 1.5),
            (work(3840, 2088), 1.5),
            (work(3840, 2064), 2.0),
        ];
        for (area, scale) in cases {
            let placement = initial_placement(area, scale);
            assert!(inside(placement.rect, area), "{area:?} @ {scale}: {placement:?}");
            assert!(placement.min.0 <= area.width && placement.min.1 <= area.height);
            assert!(placement.rect.width >= placement.min.0 && placement.rect.height >= placement.min.1);
        }
    }

    #[test]
    fn small_screens_maximize_large_ones_center() {
        assert!(initial_placement(work(1366, 728), 1.0).maximize);
        assert!(initial_placement(work(1920, 1040), 1.0).maximize);
        assert!(initial_placement(work(1920, 1020), 1.5).maximize);
        let big = initial_placement(work(2560, 1392), 1.0);
        assert!(!big.maximize);
        assert_eq!((big.rect.width, big.rect.height), (1680, 1050));
        assert_eq!((big.rect.x, big.rect.y), (440, 171));
        // 4K at 200 % behaves like a 1080p screen.
        assert!(initial_placement(work(3840, 2064), 2.0).maximize);
        let uhd = initial_placement(work(3840, 2088), 1.5);
        assert!(!uhd.maximize);
        assert_eq!((uhd.rect.width, uhd.rect.height), (2520, 1575));
    }

    #[test]
    fn work_area_offset_is_respected() {
        // Secondary monitor to the left with the taskbar on top.
        let area = Rect { x: -1920, y: 40, width: 1920, height: 1040 };
        let placement = initial_placement(area, 1.0);
        assert!(inside(placement.rect, area));
    }

    #[test]
    fn clamp_moves_window_back_on_screen() {
        let area = work(1366, 728);
        // The old fixed 1680x1050 window centered on a 1366x768 screen.
        let old = Rect { x: -157, y: -141, width: 1680, height: 1050 };
        let fixed = clamp_into(old, area).unwrap();
        assert_eq!(fixed, Rect { x: 0, y: 0, width: 1366, height: 728 });
        let ok = Rect { x: 100, y: 50, width: 1100, height: 650 };
        assert_eq!(clamp_into(ok, area), None);
        let off_right = Rect { x: 900, y: 600, width: 800, height: 400 };
        assert_eq!(clamp_into(off_right, area), Some(Rect { x: 566, y: 328, width: 800, height: 400 }));
    }
}
