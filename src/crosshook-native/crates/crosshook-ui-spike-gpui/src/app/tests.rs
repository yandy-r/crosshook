//! `#[gpui::test]` tests on a headless `TestAppContext`: the real shell inside GPUI Kit's
//! Root, driven by real keystrokes through the real keymap (Tab, F6, arrows, Enter, Escape).
//! Synthetic summaries replace the core load, so no I/O runs.

use gpui::{
    AppContext as _, Entity, TestAppContext, WindowBounds, WindowHandle, WindowOptions, px, size,
};
use gpui_kit::base::Root;

use super::{Overlay, Route, Shell};
use crate::console::ring::CAPACITY;
use crate::core_bridge::Summary;
use crate::focus::{Target, Zone};

fn summaries(n: usize) -> Vec<Summary> {
    (0..n)
        .map(|i| Summary {
            name: format!("bench-game-{i:04}"),
            title: format!("Bench Game {i:04}"),
            app_id: format!("{}", 100_000 + i),
        })
        .collect()
}

/// Opens the shell in a 1600x1000 window (wide layout) through `gpui_kit::open_window`.
fn open(cx: &mut TestAppContext) -> (WindowHandle<Root>, Entity<Shell>) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        crate::actions::bind_keys(cx);
        let bounds = gpui::Bounds {
            origin: gpui::Point::default(),
            size: size(px(1600.), px(1000.)),
        };
        let (window, shell) = gpui_kit::open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            cx,
            |window, cx| {
                cx.new(|cx| {
                    let mut s = Shell::new(false, window, cx);
                    s.set_library(None, summaries(40), window, cx);
                    s
                })
            },
        )
        .expect("open window");
        (window.downcast::<Root>().expect("Root"), shell)
    })
}

fn focused(shell: &Entity<Shell>, cx: &mut TestAppContext) -> Option<Target> {
    shell.read_with(cx, |s, _| s.focus.focused.clone())
}

fn keys(win: WindowHandle<Root>, cx: &mut TestAppContext, keys: &str) {
    cx.simulate_keystrokes(win.into(), keys);
    cx.run_until_parked();
}

#[gpui::test]
fn focus_zones_dpad_and_tab_traverse_the_slice(cx: &mut TestAppContext) {
    let (win, shell) = open(cx);
    cx.run_until_parked();
    // F6 from nothing lands on the first zone (sidebar) and cycles with memory.
    keys(win, cx, "f6");
    assert_eq!(focused(&shell, cx), Some(Target::Nav(0)));
    keys(win, cx, "f6");
    assert_eq!(focused(&shell, cx).map(|t| t.zone()), Some(Zone::Content));
    // In the library, arrows walk the grid model (cards culled by the list stay reachable).
    shell.update_in_window(win, cx, |s, w, cx| s.focus_to(Target::Card(0), w, cx));
    keys(win, cx, "right");
    assert_eq!(focused(&shell, cx), Some(Target::Card(1)));
    keys(win, cx, "down");
    let cols = shell.read_with(cx, |s, _| s.cols);
    assert_eq!(focused(&shell, cx), Some(Target::Card(1 + cols)));
    keys(win, cx, "f6");
    assert_eq!(focused(&shell, cx).map(|t| t.zone()), Some(Zone::Inspector));
    keys(win, cx, "shift-f6");
    assert_eq!(
        focused(&shell, cx),
        Some(Target::Card(1 + cols)),
        "content zone remembers the last card"
    );
    // Tab is Kit Root's real tab order; it moves gpui focus and the engine follows it.
    let before = focused(&shell, cx);
    keys(win, cx, "tab");
    assert_ne!(focused(&shell, cx), before, "Tab moves focus");
    // Enter on a sidebar entry navigates (the same action the gamepad's A dispatches).
    shell.update_in_window(win, cx, |s, w, cx| s.focus_to(Target::Nav(1), w, cx));
    keys(win, cx, "enter");
    assert_eq!(shell.read_with(cx, |s, _| s.route()), Route::Settings);
    // Back/forward history: Alt+Left returns to the library, Alt+Right goes forward again.
    keys(win, cx, "alt-left");
    assert_eq!(shell.read_with(cx, |s, _| s.route()), Route::Library);
    keys(win, cx, "alt-right");
    assert_eq!(shell.read_with(cx, |s, _| s.route()), Route::Settings);
}

#[gpui::test]
fn every_settings_control_is_reachable_by_dpad(cx: &mut TestAppContext) {
    let (win, shell) = open(cx);
    shell.update(cx, |s, cx| s.navigate(Route::Settings, cx));
    cx.run_until_parked();
    shell.update_in_window(win, cx, |s, w, cx| s.focus_to(Target::Field(6), w, cx));
    let mut seen = std::collections::BTreeSet::new();
    seen.insert(6u8);
    for _ in 0..20 {
        keys(win, cx, "down");
        if let Some(Target::Field(n)) = focused(&shell, cx) {
            seen.insert(n);
        }
    }
    // The Save / Pick-file row: the vertical sweep enters it on one button; walk the row both
    // ways so the other one is reached by the D-pad as well.
    for dir in ["left", "right", "left"] {
        keys(win, cx, dir);
        if let Some(Target::Field(n)) = focused(&shell, cx) {
            seen.insert(n);
        }
    }
    let all: std::collections::BTreeSet<u8> = (0..=14).collect();
    let missing: Vec<_> = all.difference(&seen).collect();
    assert!(
        missing.is_empty(),
        "fields not reachable by D-pad: {missing:?}"
    );
}

#[gpui::test]
fn modal_traps_focus_and_escape_restores_opener(cx: &mut TestAppContext) {
    let (win, shell) = open(cx);
    cx.run_until_parked();
    shell.update_in_window(win, cx, |s, w, cx| s.focus_to(Target::Card(3), w, cx));
    shell.update_in_window(win, cx, |s, w, cx| s.open_overlay(Overlay::Modal, w, cx));
    cx.run_until_parked();
    assert_eq!(focused(&shell, cx), Some(Target::Overlay(0)));
    // D-pad stays inside the dialog; zone cycling is disabled while it is open.
    keys(win, cx, "right");
    assert_eq!(focused(&shell, cx), Some(Target::Overlay(1)));
    keys(win, cx, "right");
    assert_eq!(
        focused(&shell, cx),
        Some(Target::Overlay(1)),
        "no exit right"
    );
    keys(win, cx, "f6");
    assert_eq!(focused(&shell, cx).map(|t| t.zone()), Some(Zone::Overlay));
    // Tab is trapped by Kit's dialog focus trap.
    for _ in 0..6 {
        keys(win, cx, "tab");
        let zone = focused(&shell, cx).map(|t| t.zone());
        assert_eq!(zone, Some(Zone::Overlay), "Tab left the modal");
    }
    // Escape (gamepad B dispatches the same Back action) closes and restores the opener.
    keys(win, cx, "escape");
    assert!(shell.read_with(cx, |s, _| s.overlay.is_none()));
    assert_eq!(focused(&shell, cx), Some(Target::Card(3)));
}

#[gpui::test]
fn palette_navigates_and_settings_page_scrolls(cx: &mut TestAppContext) {
    let (win, shell) = open(cx);
    cx.run_until_parked();
    keys(win, cx, "ctrl-k");
    assert!(
        shell.read_with(cx, |s, _| s.overlay.is_some()),
        "palette opened"
    );
    cx.simulate_input(win.into(), "settings");
    cx.run_until_parked();
    let focus = cx
        .update_window(win.into(), |_, w, cx| {
            shell.read(cx).widget_target_dbg(w, cx)
        })
        .ok()
        .flatten();
    assert_eq!(focus, Some(Target::Palette), "the palette input has focus");
    // `simulate_keystrokes("enter")` reaches a focused Kit input as IME newline text, not a
    // key-down (GPUI Kit TESTING.md), so dispatch what a real Enter resolves to in the dialog.
    cx.dispatch_action(
        win.into(),
        gpui_component::dialog::Confirm { secondary: false },
    );
    cx.run_until_parked();
    let (ov, route) = shell.read_with(cx, |s, _| (s.overlay, s.route()));
    assert!(ov.is_none(), "palette closed (route={route:?})");
    assert_eq!(shell.read_with(cx, |s, _| s.route()), Route::Settings);
    cx.update_window(win.into(), |_, w, _| w.refresh()).ok();
    cx.run_until_parked();
    let max = shell.read_with(cx, |s, _| s.settings_scroll.max_offset().y);
    assert!(
        max > gpui::px(0.),
        "settings page must be scrollable, max offset {max:?}"
    );
}

#[gpui::test]
fn console_ring_stays_bounded(cx: &mut TestAppContext) {
    let (_win, shell) = open(cx);
    let console = shell.read_with(cx, |s, _| s.console.clone());
    for batch in 0..50 {
        let lines = (0..1000).map(|i| format!("line {batch}-{i}")).collect();
        console.update(cx, |c, cx| c.append(lines, cx));
    }
    cx.run_until_parked();
    console.read_with(cx, |c, _| {
        assert_eq!(c.console.ring.len(), CAPACITY);
        assert_eq!(c.console.ring.dropped(), 40_000);
        assert_eq!(c.console.ring.get(CAPACITY - 1), Some("line 49-999"));
    });
}

/// Small helper: run a closure against the shell inside its window.
trait UpdateInWindow {
    fn update_in_window(
        &self,
        win: WindowHandle<Root>,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Shell, &mut gpui::Window, &mut gpui::Context<Shell>),
    );
}

impl UpdateInWindow for Entity<Shell> {
    fn update_in_window(
        &self,
        win: WindowHandle<Root>,
        cx: &mut TestAppContext,
        f: impl FnOnce(&mut Shell, &mut gpui::Window, &mut gpui::Context<Shell>),
    ) {
        cx.update_window(win.into(), |_, window, cx| {
            self.update(cx, |s, cx| f(s, window, cx))
        })
        .expect("window");
        cx.run_until_parked();
    }
}
