//! The lock screen's layout: widgets like the desktop's, in
//! `lock-widgets.toml` next to `widgets.toml`, arranged with the same
//! editor (`mochi ipc widgets edit-lock`), and drawn by the lock screen.
//!
//! Only widgets whose offer lists the `lock` place go there. The password
//! field is required: it can't be removed, and a layout without one gets
//! one back. A widget whose `output` is empty shows on every monitor, which
//! is how the default layout is.

use crate::layout::{Anchor, Layout, Placed};

pub const FILE: &str = "lock-widgets.toml";

/// The lock module's password field, the one widget the lock screen needs.
pub const FIELD: (&str, &str) = ("lock", "field");

/// The modules whose widgets may go on the lock screen: Mochi's own, whose
/// views the lock screen has and whose data it's given. A plugin's offer
/// listing the `lock` place doesn't count, yet.
pub const MODULES: [&str; 6] = ["widgets", "weather", "media", "battery", "timer", "lock"];

fn widget(
    id: &str,
    (module, name): (&str, &str),
    variant: Option<&str>,
    anchor: Anchor,
    (x, y): (i32, i32),
    (width, height): (u32, u32),
) -> Placed {
    Placed {
        id: id.into(),
        module: module.into(),
        widget: name.into(),
        variant: variant.map(str::to_owned),
        // Every monitor.
        output: String::new(),
        anchor,
        x,
        y,
        width,
        height,
        z: 0,
        settings: toml::Table::new(),
    }
}

/// The password field where the default layout has it: right of the
/// middle.
pub fn field() -> Placed {
    widget("lock-field", FIELD, None, Anchor::Right, (-8, 0), (24, 4))
}

/// The layout before any arranging: the clock, big, on the left with the
/// weather under it; what's playing over the field on the right; the
/// battery in the corner.
pub fn default_layout() -> Layout {
    Layout {
        widgets: vec![
            widget(
                "lock-clock",
                ("widgets", "clock"),
                Some("stacked"),
                Anchor::Left,
                (8, -2),
                (20, 26),
            ),
            widget(
                "lock-weather",
                ("weather", "current"),
                None,
                Anchor::Left,
                (8, 15),
                (20, 6),
            ),
            widget(
                "lock-media",
                ("media", "now-playing"),
                Some("card"),
                Anchor::Right,
                (-8, -8),
                (24, 9),
            ),
            field(),
            widget(
                "lock-battery",
                ("battery", "level"),
                Some("ring"),
                Anchor::TopRight,
                (-4, 4),
                (9, 10),
            ),
        ],
        ..Layout::default()
    }
}

/// Gives a layout its password field back if it lost it. `true` when it
/// did.
pub fn ensure_field(layout: &mut Layout) -> bool {
    let has = layout
        .widgets
        .iter()
        .any(|widget| (widget.module.as_str(), widget.widget.as_str()) == FIELD);
    if !has {
        let mut field = field();
        // Not an id another widget has.
        if layout.widgets.iter().any(|widget| widget.id == field.id) {
            field.id = layout.new_id([]);
        }
        layout.widgets.push(field);
    }
    !has
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_has_a_field_on_every_monitor() {
        let mut layout = default_layout();
        assert!(!ensure_field(&mut layout));
        assert!(layout.widgets.iter().all(|widget| widget.output.is_empty()));
        // Any monitor shows them all.
        assert_eq!(layout.monitor_for("DP-3"), "");
    }

    #[test]
    fn a_layout_without_its_field_gets_it_back() {
        let mut layout = default_layout();
        layout.widgets.retain(|widget| widget.widget != "field");
        assert!(ensure_field(&mut layout));
        assert_eq!(
            layout
                .widgets
                .iter()
                .filter(|widget| widget.widget == "field")
                .count(),
            1
        );
        assert!(Layout::parse(&layout.to_toml()).is_ok());
    }
}
