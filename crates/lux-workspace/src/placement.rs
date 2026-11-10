//! Where a saved window goes when its monitor may have changed.
//!
//! Coordinates are physical pixels, origin at the top left of the desktop,
//! the same space `winit` reports for `outer_position`. The function does
//! no I/O and does not know about a window: the desktop layer measures,
//! this layer decides.

use serde::{Deserialize, Serialize};

/// One window as it was when the editor last quit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowFrame {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default)]
    pub monitor_name: String,
    #[serde(default)]
    pub monitor_x: i32,
    #[serde(default)]
    pub monitor_y: i32,
    /// `scale_factor * 1000`, so the value stays an integer.
    #[serde(default = "default_scale")]
    pub scale_millis: u32,
    #[serde(default)]
    pub fullscreen: bool,
    #[serde(default)]
    pub maximized: bool,
}

fn default_width() -> u32 {
    1000
}

fn default_height() -> u32 {
    700
}

fn default_scale() -> u32 {
    1000
}

impl Default for WindowFrame {
    fn default() -> Self {
        WindowFrame {
            x: 0,
            y: 0,
            width: default_width(),
            height: default_height(),
            monitor_name: String::new(),
            monitor_x: 0,
            monitor_y: 0,
            scale_millis: default_scale(),
            fullscreen: false,
            maximized: false,
        }
    }
}

/// A connected display, in the same pixel space as [`WindowFrame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorRect {
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// The frame to create. `width` and `height` are the saved inner size,
/// except when `fullscreen` or `maximized` is set, in which case they are
/// the destination monitor's size.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedWindow {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
    pub maximized: bool,
}

/// Puts `saved` on a display that exists now.
///
/// A monitor matched by name keeps the window's offset from that
/// monitor's origin, so a display that moved stays under the window.
/// A named monitor that is gone, and whose saved rectangle misses every
/// connected display, moves the window onto the largest visible one.
/// Fullscreen is kept only on the monitor that still has that name.
/// Maximized fills whichever display the window lands on.
pub fn restore_window(saved: &WindowFrame, monitors: &[MonitorRect]) -> PlacedWindow {
    let width = saved.width.max(1);
    let height = saved.height.max(1);
    if monitors.is_empty() {
        return PlacedWindow {
            x: saved.x,
            y: saved.y,
            width,
            height,
            fullscreen: saved.fullscreen,
            maximized: saved.maximized && !saved.fullscreen,
        };
    }

    if let Some(monitor) = named(saved, monitors) {
        if saved.fullscreen || saved.maximized {
            return fill(monitor, saved.fullscreen);
        }
        return clamp(
            PlacedWindow {
                x: monitor.x + (saved.x - saved.monitor_x),
                y: monitor.y + (saved.y - saved.monitor_y),
                width,
                height,
                fullscreen: false,
                maximized: false,
            },
            monitor,
        );
    }

    if let Some(monitor) = monitors
        .iter()
        .find(|monitor| contains(monitor, saved.x, saved.y, width, height))
    {
        if saved.maximized {
            return fill(monitor, false);
        }
        return PlacedWindow {
            x: saved.x,
            y: saved.y,
            width,
            height,
            fullscreen: false,
            maximized: false,
        };
    }

    let dest = largest(monitors);
    if saved.maximized {
        return fill(dest, false);
    }
    clamp(
        PlacedWindow {
            x: dest.x.saturating_add(40),
            y: dest.y.saturating_add(40),
            width: width.min(dest.width),
            height: height.min(dest.height),
            fullscreen: false,
            maximized: false,
        },
        dest,
    )
}

fn named<'a>(saved: &WindowFrame, monitors: &'a [MonitorRect]) -> Option<&'a MonitorRect> {
    if saved.monitor_name.is_empty() {
        return None;
    }
    monitors
        .iter()
        .find(|monitor| monitor.name == saved.monitor_name)
}

fn fill(monitor: &MonitorRect, fullscreen: bool) -> PlacedWindow {
    PlacedWindow {
        x: monitor.x,
        y: monitor.y,
        width: monitor.width.max(1),
        height: monitor.height.max(1),
        fullscreen,
        maximized: !fullscreen,
    }
}

fn largest(monitors: &[MonitorRect]) -> &MonitorRect {
    monitors
        .iter()
        .max_by_key(|monitor| monitor.width as u64 * monitor.height as u64)
        .expect("monitors is not empty")
}

fn contains(monitor: &MonitorRect, x: i32, y: i32, width: u32, height: u32) -> bool {
    let right = x.saturating_add(width as i32);
    let bottom = y.saturating_add(height as i32);
    x >= monitor.x
        && y >= monitor.y
        && right <= monitor.x.saturating_add(monitor.width as i32)
        && bottom <= monitor.y.saturating_add(monitor.height as i32)
}

fn clamp(mut placed: PlacedWindow, monitor: &MonitorRect) -> PlacedWindow {
    if placed.width > monitor.width {
        placed.width = monitor.width.max(1);
    }
    if placed.height > monitor.height {
        placed.height = monitor.height.max(1);
    }
    if placed.x < monitor.x {
        placed.x = monitor.x;
    }
    if placed.y < monitor.y {
        placed.y = monitor.y;
    }
    let right = monitor.x.saturating_add(monitor.width as i32);
    let bottom = monitor.y.saturating_add(monitor.height as i32);
    let far_x = placed.x.saturating_add(placed.width as i32);
    if far_x > right {
        placed.x -= far_x - right;
    }
    let far_y = placed.y.saturating_add(placed.height as i32);
    if far_y > bottom {
        placed.y -= far_y - bottom;
    }
    placed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(x: i32, y: i32, width: u32, height: u32, name: &str, mx: i32, my: i32) -> WindowFrame {
        WindowFrame {
            x,
            y,
            width,
            height,
            monitor_name: name.to_string(),
            monitor_x: mx,
            monitor_y: my,
            scale_millis: 1000,
            fullscreen: false,
            maximized: false,
        }
    }

    fn built_in() -> MonitorRect {
        MonitorRect {
            name: "Built-in".to_string(),
            x: 0,
            y: 0,
            width: 1440,
            height: 900,
        }
    }

    #[test]
    fn the_same_monitor_keeps_the_frame() {
        let saved = frame(120, 80, 640, 480, "Color LCD", 0, 0);
        let monitors = [MonitorRect {
            name: "Color LCD".to_string(),
            x: 0,
            y: 0,
            width: 1024,
            height: 768,
        }];
        let placed = restore_window(&saved, &monitors);
        assert_eq!(placed.x, 120);
        assert_eq!(placed.y, 80);
        assert_eq!(placed.width, 640);
        assert_eq!(placed.height, 480);
        assert!(!placed.fullscreen);
        assert!(!placed.maximized);
    }

    #[test]
    fn a_moved_monitor_keeps_the_offset_from_its_origin() {
        let saved = frame(100, 50, 800, 600, "Dell", 0, 0);
        let monitors = [MonitorRect {
            name: "Dell".to_string(),
            x: 1920,
            y: 0,
            width: 1920,
            height: 1080,
        }];
        let placed = restore_window(&saved, &monitors);
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (2020, 50, 800, 600)
        );
    }

    #[test]
    fn a_disconnected_monitor_moves_the_window_onto_a_visible_one() {
        let saved = frame(2400, 120, 800, 600, "HDMI", 1920, 0);
        let placed = restore_window(&saved, &[built_in()]);
        assert!(placed.x >= 0);
        assert!(placed.y >= 0);
        assert!(placed.x + placed.width as i32 <= 1440);
        assert!(placed.y + placed.height as i32 <= 900);
        assert_ne!(placed.x, 2400);
        assert!(!placed.fullscreen);
        assert!(!placed.maximized);
    }

    #[test]
    fn a_window_larger_than_the_remaining_monitor_is_shrunk_to_fit() {
        let saved = frame(4000, 100, 2000, 2000, "Gone", 3000, 0);
        let small = MonitorRect {
            name: "Built-in".to_string(),
            x: 0,
            y: 0,
            width: 800,
            height: 600,
        };
        let placed = restore_window(&saved, &[small]);
        assert!(placed.width <= 800);
        assert!(placed.height <= 600);
        assert!(placed.x >= 0 && placed.y >= 0);
        assert!(placed.x + placed.width as i32 <= 800);
        assert!(placed.y + placed.height as i32 <= 600);
    }

    #[test]
    fn fullscreen_stays_on_the_named_monitor() {
        let mut saved = frame(10, 10, 800, 600, "Built-in", 0, 0);
        saved.fullscreen = true;
        let placed = restore_window(&saved, &[built_in()]);
        assert!(placed.fullscreen);
        assert!(!placed.maximized);
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (0, 0, 1440, 900)
        );
    }

    #[test]
    fn maximized_on_a_missing_monitor_fills_a_visible_one() {
        let mut saved = frame(1920, 0, 1920, 1080, "HDMI", 1920, 0);
        saved.maximized = true;
        let placed = restore_window(&saved, &[built_in()]);
        assert!(placed.maximized);
        assert!(!placed.fullscreen);
        assert_eq!(
            (placed.x, placed.y, placed.width, placed.height),
            (0, 0, 1440, 900)
        );
    }
}
