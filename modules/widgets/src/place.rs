//! Where widgets are on a screen, in pixels, the way the editor's
//! `Place.js` works it out, for putting a new widget in the first free
//! spot: a widget's anchor point sits at the same point of the screen,
//! moved by its offsets in grid cells.

use crate::layout::{Anchor, Placed};

/// Free space kept between the screen's edges and a new widget, in cells.
const MARGIN: f64 = 2.0;
/// Free space kept between a new widget and the others, in cells.
const GAP: f64 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    fn overlaps(&self, other: &Rect, gap: f64) -> bool {
        self.x < other.x + other.width + gap
            && other.x < self.x + self.width + gap
            && self.y < other.y + other.height + gap
            && other.y < self.y + self.height + gap
    }
}

impl Anchor {
    /// How far across and down its point is: 0, 0.5 or 1.
    fn factors(self) -> (f64, f64) {
        let name = self.as_str();
        let across = if name.ends_with("left") {
            0.0
        } else if name.ends_with("right") {
            1.0
        } else {
            0.5
        };
        let down = if name.starts_with("top") {
            0.0
        } else if name.starts_with("bottom") {
            1.0
        } else {
            0.5
        };
        (across, down)
    }
}

/// Where a widget anchored at `anchor`, `x` and `y` cells from it and
/// `size` cells big, is on a screen `screen` pixels big.
fn rect_at(
    anchor: Anchor,
    x: i32,
    y: i32,
    size: (u32, u32),
    cell: f64,
    screen: (f64, f64),
) -> Rect {
    let (across, down) = anchor.factors();
    let width = f64::from(size.0) * cell;
    let height = f64::from(size.1) * cell;
    Rect {
        x: across * (screen.0 - width) + f64::from(x) * cell,
        y: down * (screen.1 - height) + f64::from(y) * cell,
        width,
        height,
    }
}

/// A placed widget's rectangle.
pub fn rect(widget: &Placed, cell: f64, screen: (f64, f64)) -> Rect {
    rect_at(
        widget.anchor,
        widget.x,
        widget.y,
        (widget.width, widget.height),
        cell,
        screen,
    )
}

/// The anchor and offsets for a widget at `at`: the anchor of the third of
/// the screen its middle is in, so it keeps to that side when the screen
/// changes size.
pub fn place(at: Rect, cell: f64, screen: (f64, f64)) -> (Anchor, i32, i32) {
    let middle = (at.x + at.width / 2.0, at.y + at.height / 2.0);
    let third = |value: f64, length: f64, low: &'static str, high: &'static str| {
        if value < length / 3.0 {
            low
        } else if value > length * 2.0 / 3.0 {
            high
        } else {
            ""
        }
    };
    let across = third(middle.0, screen.0, "left", "right");
    let down = third(middle.1, screen.1, "top", "bottom");
    let name = match (down, across) {
        ("", "") => "center".to_owned(),
        (down, "") => down.to_owned(),
        ("", across) => across.to_owned(),
        (down, across) => format!("{down}-{across}"),
    };
    let anchor = Anchor::parse(&name).unwrap_or_default();
    let (fx, fy) = anchor.factors();
    #[allow(clippy::cast_possible_truncation)]
    let offset = |start: f64, factor: f64, length: f64, size: f64| {
        ((start - factor * (length - size)) / cell).round() as i32
    };
    (
        anchor,
        offset(at.x, fx, screen.0, at.width),
        offset(at.y, fy, screen.1, at.height),
    )
}

/// The first spot on a screen `screen` pixels big where a widget `size`
/// cells big fits without touching `others`: down the left edge first,
/// then a cell further right each time, keeping clear of the edges. `None`
/// when there's no room.
pub fn first_free(
    others: &[Rect],
    size: (u32, u32),
    cell: f64,
    screen: (f64, f64),
) -> Option<(Anchor, i32, i32)> {
    let width = f64::from(size.0) * cell;
    let height = f64::from(size.1) * cell;
    let margin = MARGIN * cell;
    let mut x = margin;
    while x + width <= screen.0 - margin {
        let mut y = margin;
        while y + height <= screen.1 - margin {
            let spot = place(
                Rect {
                    x,
                    y,
                    width,
                    height,
                },
                cell,
                screen,
            );
            // Where it's saved, a whole number of cells from its anchor.
            let saved = rect_at(spot.0, spot.1, spot.2, size, cell, screen);
            let clear = others
                .iter()
                .all(|other| !saved.overlaps(other, GAP * cell));
            let inside = saved.x >= 0.0
                && saved.y >= 0.0
                && saved.x + width <= screen.0
                && saved.y + height <= screen.1;
            if clear && inside {
                return Some(spot);
            }
            y += cell;
        }
        x += cell;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (f64, f64) = (1600.0, 1000.0);
    const CELL: f64 = 16.0;

    fn widget(anchor: Anchor, x: i32, y: i32, size: (u32, u32)) -> Placed {
        Placed {
            id: "w1".into(),
            module: "widgets".into(),
            widget: "clock".into(),
            variant: None,
            output: "DP-1".into(),
            anchor,
            x,
            y,
            width: size.0,
            height: size.1,
            z: 0,
            settings: toml::Table::new(),
        }
    }

    #[test]
    fn places_by_the_third_of_the_screen() {
        let at = |x, y| Rect {
            x,
            y,
            width: 160.0,
            height: 80.0,
        };
        assert_eq!(place(at(32.0, 32.0), CELL, SCREEN), (Anchor::TopLeft, 2, 2));
        // 1600 - 160 - 32 from the right edge: two cells in.
        assert_eq!(
            place(at(1408.0, 888.0), CELL, SCREEN),
            (Anchor::BottomRight, -2, -2)
        );
        assert_eq!(
            place(at(720.0, 460.0), CELL, SCREEN),
            (Anchor::Center, 0, 0)
        );
        // And back again.
        let back = widget(Anchor::BottomRight, -2, -2, (10, 5));
        assert_eq!(rect(&back, CELL, SCREEN), at(1408.0, 888.0));
    }

    #[test]
    fn the_first_free_spot_goes_down_the_left_edge() {
        let size = (14, 7);
        // An empty screen: its top-left corner, two cells in.
        assert_eq!(
            first_free(&[], size, CELL, SCREEN),
            Some((Anchor::TopLeft, 2, 2))
        );
        // Under one already there, a cell apart.
        let first = rect(&widget(Anchor::TopLeft, 2, 2, size), CELL, SCREEN);
        assert_eq!(
            first_free(&[first], size, CELL, SCREEN),
            Some((Anchor::TopLeft, 2, 10))
        );
        // Widgets elsewhere, anchored from other corners, don't matter.
        let corner = rect(&widget(Anchor::BottomRight, -2, -2, size), CELL, SCREEN);
        assert_eq!(
            first_free(&[corner], size, CELL, SCREEN),
            Some((Anchor::TopLeft, 2, 2))
        );
    }

    #[test]
    fn a_full_left_edge_moves_it_right() {
        // A tall widget down the whole left edge.
        let column = rect(&widget(Anchor::TopLeft, 2, 2, (14, 58)), CELL, SCREEN);
        let (anchor, x, y) = first_free(&[column], (14, 7), CELL, SCREEN).unwrap();
        assert_eq!((anchor, x, y), (Anchor::TopLeft, 17, 2));
    }

    #[test]
    fn no_room_is_none() {
        let everything = Rect {
            x: 0.0,
            y: 0.0,
            width: SCREEN.0,
            height: SCREEN.1,
        };
        assert_eq!(first_free(&[everything], (4, 4), CELL, SCREEN), None);
        assert_eq!(first_free(&[], (200, 4), CELL, SCREEN), None);
    }
}
