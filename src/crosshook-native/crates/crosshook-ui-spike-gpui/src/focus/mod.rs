//! Focus layer: targets/zones, the pure engine (ported from the iced spike), a per-frame bounds
//! registry and the `wrap` element. GPUI already has real focus (`FocusHandle`, Tab order, Kit
//! focus traps), so this layer only adds what GPUI lacks: zones with memory, spatial D-pad
//! moves, and a target identity the gamepad can address.

pub mod engine;
pub mod target;

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Bounds, FocusHandle, InteractiveElement, IntoElement, ParentElement, Pixels,
    Styled, Window, canvas, div, prelude::FluentBuilder as _, px, rgb,
};

pub use engine::{Dir, FocusState, Rect};
pub use target::{Target, Zone};

/// Targets and their window bounds, rebuilt every frame during prepaint (paint order).
#[derive(Clone, Default)]
pub struct Registry(Rc<RefCell<Vec<(Target, Rect)>>>);

impl Registry {
    pub fn clear(&self) {
        self.0.borrow_mut().clear();
    }

    pub fn push(&self, target: Target, bounds: Bounds<Pixels>) {
        let rect = Rect {
            x: bounds.origin.x.as_f32(),
            y: bounds.origin.y.as_f32(),
            width: bounds.size.width.as_f32(),
            height: bounds.size.height.as_f32(),
        };
        let mut items = self.0.borrow_mut();
        items.retain(|(t, _)| *t != target);
        items.push((target, rect));
    }

    pub fn snapshot(&self) -> Vec<(Target, Rect)> {
        self.0.borrow().clone()
    }
}

/// Wraps `child` as a focus target: registers its bounds for the D-pad engine and draws the
/// focus ring. With `handle` the wrapper is itself a Tab stop (buttons, switches, cards); with
/// `None` the child is a Kit widget that owns its focus handle (inputs, select).
pub fn wrap(
    target: Target,
    handle: Option<&FocusHandle>,
    registry: &Registry,
    window: &Window,
    child: impl IntoElement,
) -> AnyElement {
    let focused = handle.is_some_and(|h| h.is_focused(window));
    let registry = registry.clone();
    let probe = canvas(
        move |bounds, _: &mut Window, _: &mut App| registry.push(target, bounds),
        |_, _, _, _| {},
    )
    .absolute()
    .size_full();
    div()
        .relative()
        .rounded(px(6.))
        .border_2()
        .border_color(if focused {
            rgb(0x4ea1ff)
        } else {
            rgb(0x00000000)
        })
        .when_some(handle, |d, h| d.track_focus(h))
        .child(child)
        .child(probe)
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32) -> Rect {
        Rect {
            x,
            y,
            width: 100.0,
            height: 100.0,
        }
    }

    fn grid() -> Vec<(Target, Rect)> {
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
    fn spatial_moves_within_zone() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(0));
        assert_eq!(f.spatial(&items, Dir::Right), Some(Target::Card(1)));
        assert_eq!(f.spatial(&items, Dir::Down), Some(Target::Card(3)));
        assert_eq!(f.spatial(&items, Dir::Left), Some(Target::Card(2)));
        assert_eq!(f.spatial(&items, Dir::Left), None);
    }

    #[test]
    fn zone_cycle_remembers_last_focus() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(3));
        assert_eq!(f.cycle_zone(&items, true), Some(Target::InspectorAction(0)));
        assert_eq!(f.cycle_zone(&items, true), Some(Target::Nav(0)));
        assert_eq!(f.cycle_zone(&items, true), Some(Target::Card(3)));
    }

    #[test]
    fn overlay_traps_dpad_and_restores_opener() {
        let items = grid();
        let mut f = FocusState::default();
        f.set(Target::Card(1));
        let opener = f.focused.clone();
        f.overlay_open = true;
        assert_eq!(f.spatial(&items, Dir::Down), Some(Target::Overlay(0)));
        assert_eq!(f.spatial(&items, Dir::Down), Some(Target::Overlay(1)));
        assert_eq!(f.spatial(&items, Dir::Down), None);
        assert_eq!(f.cycle_zone(&items, true), None);
        f.close_overlay(opener);
        assert_eq!(f.focused, Some(Target::Card(1)));
    }
}
