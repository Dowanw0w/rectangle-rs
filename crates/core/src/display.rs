//! Display (monitor) model and cyclic ordering.

use crate::geometry::{Point, Rect};

/// Opaque platform display handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisplayId(pub u64);

/// A single monitor.
#[derive(Clone, Debug, PartialEq)]
pub struct Display {
    pub id: DisplayId,
    /// Human readable name, e.g. `\\.\DISPLAY1` on Windows or the localised
    /// name on macOS.
    pub name: String,
    /// The full display rect.
    pub bounds: Rect,
    /// The usable rect: the display minus menu bar, dock and taskbar.
    pub visible: Rect,
    /// Backing scale factor (1.0 on standard DPI, 2.0 on retina).
    pub scale: f64,
    pub is_primary: bool,
}

impl Display {
    pub fn new(id: DisplayId, name: impl Into<String>, bounds: Rect, visible: Rect) -> Self {
        Self {
            id,
            name: name.into(),
            bounds,
            visible,
            scale: 1.0,
            is_primary: false,
        }
    }

    pub fn with_scale(mut self, scale: f64) -> Self {
        self.scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        self
    }

    pub fn with_primary(mut self, is_primary: bool) -> Self {
        self.is_primary = is_primary;
        self
    }

    /// Short label for the UI, e.g. `DISPLAY1 (主显示器)`.
    pub fn label(&self) -> String {
        let name = self
            .name
            .rsplit('\\')
            .next()
            .unwrap_or(self.name.as_str())
            .to_string();
        if self.is_primary {
            format!("{name} (主) {}x{}", self.bounds.width, self.bounds.height)
        } else {
            format!("{name} {}x{}", self.bounds.width, self.bounds.height)
        }
    }
}

/// A stable, ordered collection of displays.
///
/// Ordering is by left edge then top edge, which gives "next display" a
/// predictable left-to-right, top-to-bottom walk (matching how most virtual
/// desktop arrangements are laid out) and makes cycling deterministic.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Displays {
    items: Vec<Display>,
}

impl Displays {
    pub fn new(displays: Vec<Display>) -> Self {
        let mut items = displays;
        items.sort_by(|a, b| {
            a.bounds
                .x
                .partial_cmp(&b.bounds.x)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(
                    a.bounds
                        .y
                        .partial_cmp(&b.bounds.y)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        Self { items }
    }

    pub fn items(&self) -> &[Display] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn by_id(&self, id: DisplayId) -> Option<&Display> {
        self.items.iter().find(|display| display.id == id)
    }

    pub fn primary(&self) -> Option<&Display> {
        self.items
            .iter()
            .find(|display| display.is_primary)
            .or_else(|| self.items.first())
    }

    /// The display a window is considered to belong to: the one it overlaps the
    /// most, falling back to the one containing its centre, then the primary
    /// display.
    pub fn display_for(&self, rect: Rect) -> Option<&Display> {
        if self.items.is_empty() {
            return None;
        }

        let best_overlap = self
            .items
            .iter()
            .filter_map(|display| {
                let overlap = rect.intersection(&display.bounds)?.area();
                (overlap > 0.0).then_some((display, overlap))
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        if let Some((display, _)) = best_overlap {
            return Some(display);
        }

        let center: Point = rect.center();
        self.items
            .iter()
            .find(|display| display.bounds.contains_point(center))
            .or_else(|| self.primary())
            .or_else(|| self.items.first())
    }

    /// The next (or previous) display, wrapping around at both ends.
    pub fn neighbour_of(&self, id: DisplayId, forward: bool) -> Option<&Display> {
        if self.items.is_empty() {
            return None;
        }
        let current = self.items.iter().position(|display| display.id == id)?;
        let len = self.items.len();
        let index = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.items.get(index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn display(id: u64, x: f64, y: f64, width: f64, height: f64, primary: bool) -> Display {
        Display::new(
            DisplayId(id),
            format!("DISPLAY{id}"),
            Rect::new(x, y, width, height),
            Rect::new(x, y, width, height),
        )
        .with_primary(primary)
    }

    fn three_displays() -> Displays {
        // Deliberately unsorted input.
        Displays::new(vec![
            display(3, 3440.0, 0.0, 1920.0, 1080.0, false),
            display(1, 0.0, 0.0, 1920.0, 1080.0, true),
            display(2, 1920.0, 0.0, 1520.0, 1080.0, false),
        ])
    }

    #[test]
    fn sorts_left_to_right() {
        let displays = three_displays();
        let ids: Vec<u64> = displays.items().iter().map(|d| d.id.0).collect();
        assert_eq!(ids, vec![1, 2, 3]);
    }

    #[test]
    fn primary_is_found() {
        let displays = three_displays();
        assert_eq!(displays.primary().unwrap().id, DisplayId(1));
    }

    #[test]
    fn cycles_forwards_and_backwards() {
        let displays = three_displays();
        assert_eq!(
            displays.neighbour_of(DisplayId(1), true).unwrap().id,
            DisplayId(2)
        );
        assert_eq!(
            displays.neighbour_of(DisplayId(2), true).unwrap().id,
            DisplayId(3)
        );
        assert_eq!(
            displays.neighbour_of(DisplayId(3), true).unwrap().id,
            DisplayId(1),
            "must wrap around"
        );
        assert_eq!(
            displays.neighbour_of(DisplayId(1), false).unwrap().id,
            DisplayId(3),
            "must wrap around backwards"
        );
    }

    #[test]
    fn single_display_wraps_to_itself() {
        let displays = Displays::new(vec![display(1, 0.0, 0.0, 1920.0, 1080.0, true)]);
        assert_eq!(
            displays.neighbour_of(DisplayId(1), true).unwrap().id,
            DisplayId(1)
        );
    }

    #[test]
    fn display_for_picks_largest_overlap() {
        let displays = three_displays();
        // Mostly on display 2, slightly overlapping display 1.
        let window = Rect::new(1800.0, 100.0, 600.0, 400.0);
        assert_eq!(displays.display_for(window).unwrap().id, DisplayId(2));
    }

    #[test]
    fn display_for_falls_back_to_centre() {
        let displays = Displays::new(vec![
            display(1, 0.0, 0.0, 1920.0, 1080.0, true),
            display(2, 1920.0, 0.0, 1920.0, 1080.0, false),
        ]);
        // A zero sized rect straddling the bezel: no overlap area at all.
        let window = Rect::new(1920.0, 500.0, 0.0, 0.0);
        assert_eq!(displays.display_for(window).unwrap().id, DisplayId(2));
    }

    #[test]
    fn display_for_falls_back_to_primary_when_off_screen() {
        let displays = three_displays();
        let window = Rect::new(-5000.0, -5000.0, 100.0, 100.0);
        assert_eq!(displays.display_for(window).unwrap().id, DisplayId(1));
    }

    #[test]
    fn display_for_handles_negative_origins() {
        let displays = Displays::new(vec![
            display(1, 0.0, 0.0, 1920.0, 1080.0, true),
            display(2, -1920.0, 0.0, 1920.0, 1080.0, false),
        ]);
        let window = Rect::new(-1500.0, 100.0, 500.0, 500.0);
        assert_eq!(displays.display_for(window).unwrap().id, DisplayId(2));
        // "Next" from the primary wraps to the display on the left.
        assert_eq!(
            displays.neighbour_of(DisplayId(1), false).unwrap().id,
            DisplayId(2)
        );
    }

    #[test]
    fn labels_are_compact() {
        let d = Display::new(
            DisplayId(1),
            "\\\\?\\DISPLAY1",
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
            Rect::new(0.0, 0.0, 1920.0, 1080.0),
        )
        .with_primary(true);
        assert_eq!(d.label(), "DISPLAY1 (主) 1920x1080");
    }

    #[test]
    fn empty_collection_is_safe() {
        let displays = Displays::default();
        assert!(displays.display_for(Rect::default()).is_none());
        assert!(displays.neighbour_of(DisplayId(1), true).is_none());
        assert!(displays.primary().is_none());
    }
}
