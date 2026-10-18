//! Window placement math.
//!
//! Two families, deliberately kept apart:
//!
//! * [`MoveAnchor`] — *move only*. The window keeps its size and is placed at a
//!   corner / edge / centre of the work area. These are the actions bound by
//!   default, because "move a window somewhere" must never resize it.
//! * [`TileLayout`] — *resize*. Quarters and halves, opt-in actions that are not
//!   bound to a hotkey by default.
//!
//! Only `tile-*`, `maximize` and `restore` are allowed to change a window's size.

use crate::geometry::{Point, Rect};

/// Move-only anchors inside the work area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

/// Resizing layouts: quarters and halves of the work area.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TileLayout {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    LeftHalf,
    RightHalf,
    TopHalf,
    BottomHalf,
}

impl MoveAnchor {
    pub fn label(self) -> &'static str {
        match self {
            MoveAnchor::TopLeft => "左上角",
            MoveAnchor::TopRight => "右上角",
            MoveAnchor::BottomLeft => "左下角",
            MoveAnchor::BottomRight => "右下角",
            MoveAnchor::Left => "靠左",
            MoveAnchor::Right => "靠右",
            MoveAnchor::Top => "靠上",
            MoveAnchor::Bottom => "靠下",
            MoveAnchor::Center => "居中",
        }
    }
}

impl TileLayout {
    pub fn label(self) -> &'static str {
        match self {
            TileLayout::TopLeft => "左上四分之一",
            TileLayout::TopRight => "右上四分之一",
            TileLayout::BottomLeft => "左下四分之一",
            TileLayout::BottomRight => "右下四分之一",
            TileLayout::LeftHalf => "左半屏",
            TileLayout::RightHalf => "右半屏",
            TileLayout::TopHalf => "上半屏",
            TileLayout::BottomHalf => "下半屏",
        }
    }
}

/// Fraction of the work area the "restore" fallback shrinks a maximized window to.
const RESTORE_FALLBACK_SCALE: f64 = 0.8;

/// Where a move-only action puts the window.
///
/// The returned rect has **the same size** as `current`; only `x`/`y` change.
/// `margin` keeps a gap to the work area edges (it never shrinks the window).
/// If the window is wider/taller than the work area the anchored edge wins and
/// the window overflows; the "centred" axis then falls back to the start edge so
/// the title bar stays reachable.
pub fn move_frame(anchor: MoveAnchor, work_area: Rect, current: Rect, margin: f64) -> Rect {
    let area = work_area.inset(margin);
    let width = current.width.max(1.0);
    let height = current.height.max(1.0);

    let centered_x = |area: Rect| {
        if width >= area.width {
            area.x
        } else {
            area.x + (area.width - width) / 2.0
        }
    };
    let centered_y = |area: Rect| {
        if height >= area.height {
            area.y
        } else {
            area.y + (area.height - height) / 2.0
        }
    };

    let (x, y) = match anchor {
        MoveAnchor::TopLeft => (area.x, area.y),
        MoveAnchor::TopRight => (area.right() - width, area.y),
        MoveAnchor::BottomLeft => (area.x, area.bottom() - height),
        MoveAnchor::BottomRight => (area.right() - width, area.bottom() - height),
        MoveAnchor::Left => (area.x, centered_y(area)),
        MoveAnchor::Right => (area.right() - width, centered_y(area)),
        MoveAnchor::Top => (centered_x(area), area.y),
        MoveAnchor::Bottom => (centered_x(area), area.bottom() - height),
        MoveAnchor::Center => (centered_x(area), centered_y(area)),
    };

    // Only the position is rounded: rounding the size would be a resize.
    Rect {
        x: x.round(),
        y: y.round(),
        width,
        height,
    }
}

/// Frame of a resizing layout (quarter / half of the work area).
pub fn tile_frame(layout: TileLayout, work_area: Rect, margin: f64) -> Rect {
    let area = work_area;
    let half_w = area.width / 2.0;
    let half_h = area.height / 2.0;

    let target = match layout {
        TileLayout::TopLeft => Rect::new(area.x, area.y, half_w, half_h),
        TileLayout::TopRight => Rect::new(area.x + half_w, area.y, half_w, half_h),
        TileLayout::BottomLeft => Rect::new(area.x, area.y + half_h, half_w, half_h),
        TileLayout::BottomRight => Rect::new(area.x + half_w, area.y + half_h, half_w, half_h),
        TileLayout::LeftHalf => Rect::new(area.x, area.y, half_w, area.height),
        TileLayout::RightHalf => Rect::new(area.x + half_w, area.y, half_w, area.height),
        TileLayout::TopHalf => Rect::new(area.x, area.y, area.width, half_h),
        TileLayout::BottomHalf => Rect::new(area.x, area.y + half_h, area.width, half_h),
    };

    target.inset(margin).clamp_inside(&area).rounded()
}

/// Does `frame` (roughly) fill `work_area`?
///
/// Used by "cancel maximize" for windows that were maximized by the OS instead of
/// by rectangle-rs, so there is no remembered frame to go back to.
pub fn fills_work_area(frame: Rect, work_area: Rect, tolerance: f64) -> bool {
    frame.roughly_equals(&work_area, tolerance)
}

/// The frame [`crate::Action::Restore`] falls back to when no previous frame was
/// remembered: shrink a work-area filling window to 80% and centre it.
pub fn restore_fallback_frame(current: Rect, work_area: Rect, margin: f64) -> Rect {
    let area = work_area.inset(margin);
    let width = (area.width * RESTORE_FALLBACK_SCALE).max(1.0);
    let height = (area.height * RESTORE_FALLBACK_SCALE).max(1.0);
    let center: Point = area.center();
    Rect {
        x: center.x - width / 2.0,
        y: center.y - height / 2.0,
        width: width.min(current.width).max(1.0),
        height: height.min(current.height).max(1.0),
    }
    .clamp_inside(&work_area)
    .rounded()
}

/// Map a window onto another display **without changing its size**.
///
/// The window keeps its offset relative to the display centre (as a fraction of
/// the display size), so a top-left corner window stays in the top-left corner on
/// the other monitor. If the window does not fit into the target work area it is
/// aligned to its top-left corner instead of being scaled down.
pub fn frame_on_target_display(current: Rect, source: Rect, target: Rect) -> Rect {
    let width = current.width.max(1.0);
    let height = current.height.max(1.0);

    let (x, y) = if source.width < 1.0 || source.height < 1.0 {
        (current.x, current.y)
    } else {
        let fraction_x = (current.center().x - source.center().x) / source.width;
        let fraction_y = (current.center().y - source.center().y) / source.height;
        let center = Point::new(
            target.center().x + fraction_x * target.width,
            target.center().y + fraction_y * target.height,
        );
        (center.x - width / 2.0, center.y - height / 2.0)
    };

    let rect = Rect {
        x,
        y,
        width,
        height,
    };
    let clamped = clamp_position(rect, target);
    Rect {
        x: clamped.x.round(),
        y: clamped.y.round(),
        width,
        height,
    }
}

/// Keep the rect's position inside `container`, never touching its size.
fn clamp_position(rect: Rect, container: Rect) -> Rect {
    let x = if rect.width <= container.width {
        rect.x.clamp(container.x, container.right() - rect.width)
    } else {
        container.x
    };
    let y = if rect.height <= container.height {
        rect.y.clamp(container.y, container.bottom() - rect.height)
    } else {
        container.y
    };
    Rect { x, y, ..rect }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::new(0.0, 25.0, 1920.0, 1055.0)
    }

    /// A normal, smaller-than-the-work-area window.
    fn current() -> Rect {
        Rect::new(200.0, 200.0, 800.0, 600.0)
    }

    #[test]
    fn moving_to_corners_keeps_the_size() {
        let cases: [(MoveAnchor, f64, f64); 5] = [
            (MoveAnchor::TopLeft, 0.0, 25.0),
            (MoveAnchor::TopRight, 1120.0, 25.0),
            (MoveAnchor::BottomLeft, 0.0, 480.0),
            (MoveAnchor::BottomRight, 1120.0, 480.0),
            (MoveAnchor::Center, 560.0, 252.5),
        ];
        for (anchor, x, y) in cases {
            let frame = move_frame(anchor, area(), current(), 0.0);
            assert_eq!((frame.width, frame.height), (800.0, 600.0), "{anchor:?}");
            assert_eq!((frame.x, frame.y), (x, y.round()), "{anchor:?}");
        }
    }

    #[test]
    fn moving_to_edges_centres_on_the_other_axis() {
        let frame = move_frame(MoveAnchor::Left, area(), current(), 0.0);
        assert_eq!((frame.x, frame.y), (0.0, 253.0));
        let frame = move_frame(MoveAnchor::Right, area(), current(), 0.0);
        assert_eq!((frame.x, frame.y), (1120.0, 253.0));
        let frame = move_frame(MoveAnchor::Top, area(), current(), 0.0);
        assert_eq!((frame.x, frame.y), (560.0, 25.0));
        let frame = move_frame(MoveAnchor::Bottom, area(), current(), 0.0);
        assert_eq!((frame.x, frame.y), (560.0, 480.0));
        for anchor in [
            MoveAnchor::Left,
            MoveAnchor::Right,
            MoveAnchor::Top,
            MoveAnchor::Bottom,
        ] {
            let frame = move_frame(anchor, area(), current(), 0.0);
            assert_eq!((frame.width, frame.height), (800.0, 600.0), "{anchor:?}");
        }
    }

    #[test]
    fn margin_offsets_the_position_but_never_the_size() {
        let frame = move_frame(MoveAnchor::TopLeft, area(), current(), 8.0);
        assert_eq!((frame.x, frame.y), (8.0, 33.0));
        assert_eq!((frame.width, frame.height), (800.0, 600.0));

        let frame = move_frame(MoveAnchor::BottomRight, area(), current(), 8.0);
        assert_eq!((frame.x, frame.y), (1112.0, 472.0));
        assert_eq!((frame.width, frame.height), (800.0, 600.0));
    }

    #[test]
    fn oversized_windows_overflow_instead_of_being_shrunk() {
        let big = Rect::new(0.0, 0.0, 2600.0, 1400.0);
        let frame = move_frame(MoveAnchor::Center, area(), big, 0.0);
        assert_eq!((frame.width, frame.height), (2600.0, 1400.0));
        // Too large to centre: aligned to the start edge so the title bar stays
        // reachable.
        assert_eq!((frame.x, frame.y), (0.0, 25.0));

        let frame = move_frame(MoveAnchor::Right, area(), big, 0.0);
        assert_eq!((frame.width, frame.height), (2600.0, 1400.0));
        assert_eq!(frame.y, 25.0);
    }

    #[test]
    fn moving_on_a_secondary_display_with_negative_origin() {
        let secondary = Rect::new(-1920.0, 0.0, 1920.0, 1080.0);
        let frame = move_frame(MoveAnchor::TopRight, secondary, current(), 0.0);
        assert_eq!((frame.x, frame.y), (-800.0, 0.0));
        assert_eq!((frame.width, frame.height), (800.0, 600.0));
    }

    #[test]
    fn tiles_are_quarters_and_halves_of_the_work_area() {
        let a = area();
        assert_eq!(
            tile_frame(TileLayout::TopLeft, a, 0.0),
            Rect::new(0.0, 25.0, 960.0, 528.0)
        );
        assert_eq!(
            tile_frame(TileLayout::BottomRight, a, 0.0),
            Rect::new(960.0, 553.0, 960.0, 528.0)
        );
        assert_eq!(
            tile_frame(TileLayout::LeftHalf, a, 0.0),
            Rect::new(0.0, 25.0, 960.0, 1055.0)
        );
        assert_eq!(
            tile_frame(TileLayout::BottomHalf, a, 0.0),
            Rect::new(0.0, 553.0, 1920.0, 528.0)
        );
    }

    #[test]
    fn tiles_respect_the_margin() {
        let frame = tile_frame(TileLayout::LeftHalf, area(), 8.0);
        assert_eq!(frame, Rect::new(8.0, 33.0, 944.0, 1039.0));
        let left = tile_frame(TileLayout::LeftHalf, area(), 8.0);
        let right = tile_frame(TileLayout::RightHalf, area(), 8.0);
        assert_eq!(right.x - left.right(), 16.0);
    }

    #[test]
    fn fills_work_area_detects_maximized_windows() {
        assert!(fills_work_area(area(), area(), 2.0));
        assert!(fills_work_area(
            Rect::new(0.5, 25.5, 1919.0, 1054.0),
            area(),
            2.0
        ));
        assert!(!fills_work_area(current(), area(), 2.0));
    }

    #[test]
    fn restore_fallback_shrinks_and_centres() {
        let a = area();
        let frame = restore_fallback_frame(a, a, 0.0);
        assert_eq!(frame.width, 1536.0);
        assert_eq!(frame.height, 844.0);
        assert!((frame.center().x - a.center().x).abs() <= 1.0);
        assert!((frame.center().y - a.center().y).abs() <= 1.0);
    }

    #[test]
    fn restore_fallback_never_grows_a_small_window() {
        let a = area();
        let small = Rect::new(100.0, 100.0, 300.0, 200.0);
        let frame = restore_fallback_frame(small, a, 0.0);
        assert_eq!((frame.width, frame.height), (300.0, 200.0));
    }

    #[test]
    fn moving_between_displays_keeps_the_size() {
        let source = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(1920.0, 0.0, 1280.0, 720.0);
        let left_half = Rect::new(0.0, 0.0, 960.0, 1080.0);
        let moved = frame_on_target_display(left_half, source, target);
        assert_eq!((moved.width, moved.height), (960.0, 1080.0));
        // Does not fit vertically -> aligned to the top of the target display.
        assert_eq!(moved.x, 1920.0);
        assert_eq!(moved.y, 0.0);
    }

    #[test]
    fn moving_a_centred_window_keeps_it_centred() {
        let source = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(0.0, -1080.0, 1280.0, 1024.0);
        let centered = Rect::new(560.0, 290.0, 800.0, 500.0);
        let moved = frame_on_target_display(centered, source, target);
        assert_eq!((moved.width, moved.height), (800.0, 500.0));
        assert!((moved.center().x - target.center().x).abs() <= 1.0);
        assert!((moved.center().y - target.center().y).abs() <= 1.0);
    }

    #[test]
    fn moving_to_another_display_clamps_into_view() {
        let source = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let target = Rect::new(1920.0, 0.0, 1280.0, 720.0);
        // Bottom-right corner of the source display.
        let window = Rect::new(1520.0, 780.0, 400.0, 300.0);
        let moved = frame_on_target_display(window, source, target);
        assert_eq!((moved.width, moved.height), (400.0, 300.0));
        assert_eq!((moved.x, moved.y), (2800.0, 420.0));
    }

    #[test]
    fn moving_handles_degenerate_source() {
        let target = Rect::new(0.0, 0.0, 1280.0, 720.0);
        let moved = frame_on_target_display(current(), Rect::new(0.0, 0.0, 0.0, 0.0), target);
        assert_eq!((moved.width, moved.height), (800.0, 600.0));
    }
}
