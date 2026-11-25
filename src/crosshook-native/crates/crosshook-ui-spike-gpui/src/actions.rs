//! Spike actions and the keymap. Keyboard and gamepad share one path: gilrs events are turned
//! into these same actions and dispatched on the focused element (`Window::dispatch_action`),
//! so D-pad and arrow keys run identical code.
//!
//! Bindings use no key context, the lowest precedence: a focused Kit input keeps its own
//! arrows/Enter/Escape, a Select keeps Up/Down/Enter, and Kit's Root keeps Tab/Shift-Tab.

use gpui::{App, KeyBinding, actions};

use crate::focus::Dir;

actions!(
    spike,
    [
        DpadUp,
        DpadDown,
        DpadLeft,
        DpadRight,
        Confirm,
        Back,
        NextZone,
        PrevZone,
        OpenPalette,
        HistoryBack,
        HistoryForward,
    ]
);

pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("up", DpadUp, None),
        KeyBinding::new("down", DpadDown, None),
        KeyBinding::new("left", DpadLeft, None),
        KeyBinding::new("right", DpadRight, None),
        KeyBinding::new("enter", Confirm, None),
        KeyBinding::new("space", Confirm, None),
        KeyBinding::new("escape", Back, None),
        KeyBinding::new("f6", NextZone, None),
        KeyBinding::new("shift-f6", PrevZone, None),
        KeyBinding::new("ctrl-k", OpenPalette, None),
        KeyBinding::new("alt-left", HistoryBack, None),
        KeyBinding::new("alt-right", HistoryForward, None),
    ]);
}

/// The gamepad / D-pad vocabulary, shared by actions and gilrs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Move(Dir),
    Confirm,
    Back,
    NextZone,
    PrevZone,
}

pub fn boxed(nav: Nav) -> Box<dyn gpui::Action> {
    match nav {
        Nav::Move(Dir::Up) => Box::new(DpadUp),
        Nav::Move(Dir::Down) => Box::new(DpadDown),
        Nav::Move(Dir::Left) => Box::new(DpadLeft),
        Nav::Move(Dir::Right) => Box::new(DpadRight),
        Nav::Confirm => Box::new(Confirm),
        Nav::Back => Box::new(Back),
        Nav::NextZone => Box::new(NextZone),
        Nav::PrevZone => Box::new(PrevZone),
    }
}
