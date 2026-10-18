//! Rectangle/glue-free window geometry.
//!
//! All values are in "screen units": physical pixels on Windows (the process is
//! per-monitor DPI aware, so no virtualisation happens) and points on macOS.
//! The origin is the top-left corner of the primary display and `y` grows
//! downwards, which matches the coordinate space used by both the Win32
//! `SetWindowPos` API and the macOS accessibility API.
//!
//! Secondary displays may live at negative coordinates.

use std::fmt;

/// A point in global screen coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// An axis aligned rectangle in global screen coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Build a rect from two opposite corners, in any order.
    pub fn from_corners(x1: f64, y1: f64, x2: f64, y2: f64) -> Self {
        Self {
            x: x1.min(x2),
            y: y1.min(y2),
            width: (x2 - x1).abs(),
            height: (y2 - y1).abs(),
        }
    }

    pub fn right(&self) -> f64 {
        self.x + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }

    pub fn center(&self) -> Point {
        Point::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.width.is_finite()
            && self.height.is_finite()
    }

    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.height <= 0.0
    }

    /// Shrink the rect by `margin` on every side.
    ///
    /// Never collapses the rect: if the requested margin is too large the size
    /// is clamped to a single unit.
    pub fn inset(&self, margin: f64) -> Self {
        let margin = if margin.is_finite() {
            margin.max(0.0)
        } else {
            0.0
        };
        let max_x = (self.width - 1.0).max(0.0) / 2.0;
        let max_y = (self.height - 1.0).max(0.0) / 2.0;
        let mx = margin.min(max_x);
        let my = margin.min(max_y);
        Self {
            x: self.x + mx,
            y: self.y + my,
            width: (self.width - 2.0 * mx).max(1.0),
            height: (self.height - 2.0 * my).max(1.0),
        }
    }

    /// Translate the rect so that it fits completely inside `container`.
    ///
    /// Oversized rects are shrunk to the container instead of being clipped.
    pub fn clamp_inside(&self, container: &Rect) -> Self {
        let width = self.width.min(container.width);
        let height = self.height.min(container.height);
        let mut rect = Self {
            width,
            height,
            ..*self
        };
        if rect.right() > container.right() {
            rect.x = container.right() - rect.width;
        }
        if rect.x < container.x {
            rect.x = container.x;
        }
        if rect.bottom() > container.bottom() {
            rect.y = container.bottom() - rect.height;
        }
        if rect.y < container.y {
            rect.y = container.y;
        }
        rect
    }

    /// Resize around the current center, then keep the result inside `container`.
    pub fn with_size(&self, width: f64, height: f64, container: &Rect) -> Self {
        let center = self.center();
        let rect = Self {
            x: center.x - width / 2.0,
            y: center.y - height / 2.0,
            width,
            height,
        };
        rect.clamp_inside(container)
    }

    /// Move the rect so its center sits on `center`, keeping it inside `container`.
    pub fn centered_at(&self, center: Point, container: &Rect) -> Self {
        let rect = Self {
            x: center.x - self.width / 2.0,
            y: center.y - self.height / 2.0,
            ..*self
        };
        rect.clamp_inside(container)
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            None
        } else {
            Some(Rect::new(x, y, right - x, bottom - y))
        }
    }

    pub fn area(&self) -> f64 {
        if self.is_empty() {
            0.0
        } else {
            self.width * self.height
        }
    }

    pub fn contains_point(&self, point: Point) -> bool {
        point.x >= self.x && point.x < self.right() && point.y >= self.y && point.y < self.bottom()
    }

    /// Round to whole screen units.
    ///
    /// Fractional window origins lead to blurry text on Windows and slightly
    /// blurry layers on non-retina macOS displays, so every frame we hand to
    /// the platform is rounded.
    pub fn rounded(&self) -> Self {
        Self {
            x: self.x.round(),
            y: self.y.round(),
            width: self.width.round().max(1.0),
            height: self.height.round().max(1.0),
        }
    }

    /// Compare with a tolerance, ignoring sub-unit jitter.
    pub fn roughly_equals(&self, other: &Rect, tolerance: f64) -> bool {
        (self.x - other.x).abs() <= tolerance
            && (self.y - other.y).abs() <= tolerance
            && (self.width - other.width).abs() <= tolerance
            && (self.height - other.height).abs() <= tolerance
    }
}

impl fmt::Display for Rect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.0},{:.0} {:.0}x{:.0}",
            self.x, self.y, self.width, self.height
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect::new(x, y, width, height)
    }

    #[test]
    fn edges_and_center() {
        let r = rect(10.0, 20.0, 100.0, 50.0);
        assert_eq!(r.right(), 110.0);
        assert_eq!(r.bottom(), 70.0);
        assert_eq!(r.center(), Point::new(60.0, 45.0));
    }

    #[test]
    fn inset_shrinks_every_side() {
        let r = rect(0.0, 0.0, 100.0, 80.0).inset(8.0);
        assert_eq!(r, rect(8.0, 8.0, 84.0, 64.0));
    }

    #[test]
    fn inset_never_collapses() {
        let r = rect(0.0, 0.0, 10.0, 10.0).inset(50.0);
        assert!(r.width >= 1.0 && r.height >= 1.0);
        assert!(r.is_finite());
    }

    #[test]
    fn inset_ignores_negative_and_nan_margins() {
        let base = rect(0.0, 0.0, 100.0, 100.0);
        assert_eq!(base.inset(-5.0), base);
        assert_eq!(base.inset(f64::NAN), base);
    }

    #[test]
    fn clamp_inside_translates() {
        let container = rect(0.0, 0.0, 1000.0, 800.0);
        let r = rect(-50.0, 700.0, 200.0, 200.0).clamp_inside(&container);
        assert_eq!(r, rect(0.0, 600.0, 200.0, 200.0));
    }

    #[test]
    fn clamp_inside_shrinks_oversized() {
        let container = rect(0.0, 0.0, 1000.0, 800.0);
        let r = rect(0.0, 0.0, 2000.0, 1600.0).clamp_inside(&container);
        assert_eq!(r, container);
    }

    #[test]
    fn clamp_inside_handles_negative_monitors() {
        let container = rect(-1920.0, -200.0, 1920.0, 1080.0);
        let r = rect(-2000.0, 900.0, 400.0, 300.0).clamp_inside(&container);
        assert_eq!(r, rect(-1920.0, 580.0, 400.0, 300.0));
    }

    #[test]
    fn intersection_area() {
        let a = rect(0.0, 0.0, 100.0, 100.0);
        let b = rect(50.0, 50.0, 100.0, 100.0);
        assert_eq!(a.intersection(&b).unwrap().area(), 2500.0);
        assert!(a.intersection(&rect(200.0, 200.0, 10.0, 10.0)).is_none());
    }

    #[test]
    fn rounded_produces_integral_units() {
        let r = rect(10.4, 20.6, 100.2, 50.9).rounded();
        assert_eq!(r, rect(10.0, 21.0, 100.0, 51.0));
        let tiny = rect(0.0, 0.0, 0.2, 0.2).rounded();
        assert_eq!((tiny.width, tiny.height), (1.0, 1.0));
    }

    #[test]
    fn roughly_equals_tolerance() {
        let a = rect(0.0, 0.0, 100.0, 100.0);
        assert!(a.roughly_equals(&rect(0.5, -0.5, 100.4, 99.6), 1.0));
        assert!(!a.roughly_equals(&rect(0.0, 0.0, 105.0, 100.0), 1.0));
    }

    #[test]
    fn center_point_containment() {
        let r = rect(0.0, 0.0, 100.0, 100.0);
        assert!(r.contains_point(Point::new(0.0, 0.0)));
        assert!(!r.contains_point(Point::new(100.0, 50.0)));
    }
}
