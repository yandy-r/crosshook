//! Focus layer: targets/zones, the pure engine, the tree collector and the wrapper widget.

pub mod collect;
pub mod engine;
pub mod focusable;
pub mod target;

pub use engine::{Dir, FocusState};
pub use focusable::focusable;
pub use target::Target;

#[cfg(test)]
mod tests {
    use super::*;
    use iced::Rectangle;

    fn r(x: f32, y: f32) -> Rectangle {
        Rectangle {
            x,
            y,
            width: 100.0,
            height: 100.0,
        }
    }

    fn grid() -> Vec<(Target, Rectangle)> {
        vec![
            (Target::Nav(0), r(0.0, 0.0)),
            (Target::Nav(1), r(0.0, 120.0)),
            (Target::Card(0), r(200.0, 0.0)),
            (Target::Card(1), r(320.0, 0.0)),
            (Target::Card(2), r(200.0, 120.0)),
            (Target::Card(3), r(320.0, 120.0)),
            (Target::InspectorAction(0), r(600.0, 0.0)),
            (Target::Overlay(0), r(250.0, 250.0)),
            (Target::Overlay(1), r(250.0, 370.0)),
        ]
    }

    #[test]
    fn tab_cycles_non_overlay_items_in_tree_order() {
        let items = grid();
        let mut f = FocusState::default();
        let seen: Vec<_> = (0..8).map(|_| f.linear(&items, true).unwrap()).collect();
        assert_eq!(seen[0], Target::Nav(0));
        assert_eq!(seen[6], Target::InspectorAction(0));
        assert_eq!(
            seen[7],
            Target::Nav(0),
            "wraps, never enters the closed overlay"
        );
    }

    #[test]
    fn spatial_moves_within_zone() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(0));
        assert_eq!(f.spatial(&items, Dir::Right), Some(Target::Card(1)));
        assert_eq!(f.spatial(&items, Dir::Down), Some(Target::Card(3)));
        assert_eq!(f.spatial(&items, Dir::Left), Some(Target::Card(2)));
        // No card further left in the content zone: stays put (sidebar is a different zone).
        assert_eq!(f.spatial(&items, Dir::Left), None);
        assert_eq!(f.focused, Some(Target::Card(2)));
    }

    #[test]
    fn zone_cycle_remembers_last_focus() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(3));
        assert_eq!(f.cycle_zone(&items, true), Some(Target::InspectorAction(0)));
        assert_eq!(f.cycle_zone(&items, true), Some(Target::Nav(0)));
        assert_eq!(
            f.cycle_zone(&items, true),
            Some(Target::Card(3)),
            "content zone memory"
        );
        assert_eq!(f.cycle_zone(&items, false), Some(Target::Nav(0)));
    }

    #[test]
    fn overlay_traps_focus_and_restores_opener() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(1));
        let opener = f.focused.clone();
        f.overlay_open = true;
        assert_eq!(f.linear(&items, true), Some(Target::Overlay(0)));
        assert_eq!(f.linear(&items, true), Some(Target::Overlay(1)));
        assert_eq!(
            f.linear(&items, true),
            Some(Target::Overlay(0)),
            "Tab stays in the overlay"
        );
        assert_eq!(
            f.cycle_zone(&items, true),
            None,
            "zone cycling is disabled in the trap"
        );
        assert_eq!(f.spatial(&items, Dir::Down), Some(Target::Overlay(1)));
        assert_eq!(
            f.spatial(&items, Dir::Down),
            None,
            "never leaves the overlay"
        );
        f.close_overlay(opener);
        assert_eq!(f.focused, Some(Target::Card(1)));
    }
}
