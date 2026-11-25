//! Headless `iced_test` simulator tests against the real `App::view` tree: the focus targets
//! are collected from the live widget tree (wrappers + text inputs), then traversal, zone
//! cycling and the modal trap are driven through the same engine `update` uses.

use iced::Size;
use iced_test::simulator::Simulator;

use super::{App, Message, Overlay, Route};
use crate::focus::Target;
use crate::focus::collect::{Collect, CollectUnit, Items};
use crate::focus::target::Zone;
use crate::library::data::Summary;

fn app() -> App {
    let (mut app, _boot) = App::boot();
    app.summaries = (0..40)
        .map(|i| Summary {
            name: format!("g{i}"),
            title: format!("Game {i:02}"),
            app_id: format!("{i}"),
        })
        .collect();
    app.refilter();
    app.window = Size::new(1280.0, 900.0);
    app.view_w = 1280.0 - super::SIDEBAR_W - super::INSPECTOR_W;
    app.view_h = 800.0;
    app
}

fn targets(app: &App) -> Items {
    let mut sim =
        Simulator::with_size(iced_test::core::Settings::default(), app.window, app.view());
    let mut collect = Collect::default();
    sim.operate(&mut CollectUnit(&mut collect));
    collect.items
}

#[test]
fn tree_exposes_every_zone_and_culls_cards() {
    let items = targets(&app());
    for zone in [Zone::Sidebar, Zone::Content, Zone::Inspector] {
        assert!(
            items.iter().any(|(t, _)| t.zone() == zone),
            "{zone:?} has focusables"
        );
    }
    assert!(
        items.iter().any(|(t, _)| *t == Target::Search),
        "text input reports through iced"
    );
    let cards = items
        .iter()
        .filter(|(t, _)| matches!(t, Target::Card(_)))
        .count();
    assert!(
        cards > 0 && cards < 40,
        "grid is culled: {cards} of 40 cards built"
    );
}

#[test]
fn tab_reaches_every_collected_target_then_wraps() {
    let mut app = app();
    let items = targets(&app);
    let mut seen = Vec::new();
    for _ in 0..items.len() {
        seen.push(app.focus.linear(&items, true).expect("focus moves"));
    }
    let all: Vec<_> = items.iter().map(|(t, _)| t.clone()).collect();
    assert_eq!(seen, all, "Tab visits targets in tree order");
    assert_eq!(
        app.focus.linear(&items, true),
        Some(all[0].clone()),
        "wraps"
    );
}

#[test]
fn f6_cycles_zones_with_memory() {
    let mut app = app();
    let items = targets(&app);
    app.focus.set(Target::Card(5));
    let z1 = app.focus.cycle_zone(&items, true).unwrap();
    assert_eq!(z1.zone(), Zone::Inspector);
    let z2 = app.focus.cycle_zone(&items, true).unwrap();
    assert_eq!(z2.zone(), Zone::Sidebar);
    assert_eq!(app.focus.cycle_zone(&items, true), Some(Target::Card(5)));
}

#[test]
fn modal_traps_focus_and_escape_restores_opener() {
    let mut app = app();
    app.history.push(Route::Settings);
    app.focus.set(Target::Field(14));
    let _ = app.update(Message::OpenModal);
    assert!(matches!(app.overlay, Some(Overlay::Modal { .. })));
    let items = targets(&app);
    assert!(items.iter().any(|(t, _)| *t == Target::Overlay(1)));
    assert_eq!(app.focus.focused, Some(Target::Overlay(0)));
    for _ in 0..5 {
        let t = app.focus.linear(&items, true).unwrap();
        assert_eq!(t.zone(), Zone::Overlay, "Tab never leaves the modal");
    }
    assert_eq!(app.focus.cycle_zone(&items, true), None);
    let _ = app.update(Message::Escape);
    assert!(app.overlay.is_none());
    assert_eq!(
        app.focus.focused,
        Some(Target::Field(14)),
        "focus returns to the opener"
    );
}

#[test]
fn dpad_reaches_culled_cards_via_grid_model() {
    let mut app = app();
    let _ = app.update(Message::FocusTarget(Target::Card(0)));
    for _ in 0..9 {
        let _ = app.update(Message::Nav(crate::gamepad::Nav::Move(
            crate::focus::Dir::Down,
        )));
    }
    let Some(Target::Card(i)) = app.focus.focused.clone() else {
        panic!("card focus")
    };
    assert!(i >= 30, "moved down past the culled rows, at {i}");
}

#[test]
fn wheel_over_console_scrolls_the_log() {
    let mut app = app();
    app.window = Size::new(1600.0, 972.0);
    app.console.append((0..10_000).map(|i| format!("line {i}")).collect());
    let mut sim = Simulator::with_size(iced_test::core::Settings::default(), app.window, app.view());
    sim.point_at(iced::Point::new(800.0, 832.0));
    for delta in [1.0, -1.0] {
        let _ = sim.scroll(iced::mouse::ScrollDelta::Lines { x: 0.0, y: delta });
    }
    let msgs: Vec<_> = sim.into_messages().collect();
    assert!(
        msgs.iter().any(|m| matches!(m, Message::ConsoleScrolled(_))),
        "wheel over the console must scroll it; got {} messages: {:?}",
        msgs.len(),
        msgs.iter().map(|m| format!("{m:?}").chars().take(40).collect::<String>()).collect::<Vec<_>>()
    );
}
