//! Focus routing: action handlers (keyboard and gamepad share them), the D-pad engine, zone
//! cycling, activation and the overlays. GPUI moves real focus (`FocusHandle::focus`); the
//! engine decides where. Tab/Shift-Tab stay with GPUI Kit's Root (tab order + focus traps).

use gpui::{Context, Window};
use gpui_component::WindowExt as _;

use super::{Overlay, Route, Shell};
use crate::actions::*;
use crate::focus::{Dir, Target, Zone};

impl Shell {
    /// Real focus → engine state. Runs every render so Tab and clicks keep the engine in sync.
    pub fn sync_focus_from_window(&mut self, window: &Window, cx: &gpui::App) {
        let Some(focused) = window.focused(cx) else {
            return;
        };
        let target = self
            .handles
            .iter()
            .find(|(_, h)| **h == focused)
            .map(|(t, _)| t.clone())
            .or_else(|| self.widget_target(&focused, window, cx));
        if let Some(t) = target
            && self.focus.focused.as_ref() != Some(&t)
        {
            self.focus.set(t);
        }
    }

    /// Kit widgets own their handles: find which one (inputs, select) holds focus.
    fn widget_target(
        &self,
        focused: &gpui::FocusHandle,
        window: &Window,
        cx: &gpui::App,
    ) -> Option<Target> {
        use gpui::Focusable as _;
        let within = |h: gpui::FocusHandle| h == *focused || h.contains_focused(window, cx);
        if within(self.search.focus_handle(cx)) {
            return Some(Target::Search);
        }
        if within(self.palette.focus_handle(cx)) {
            return Some(Target::Palette);
        }
        if within(self.filter.focus_handle(cx)) {
            return Some(Target::Field(7));
        }
        self.numbers
            .iter()
            .position(|n| within(n.focus_handle(cx)))
            .map(|i| Target::Field(i as u8))
    }

    /// Moves real gpui focus to `target` (and keeps the engine and grid scroll in step).
    pub fn focus_to(&mut self, target: Target, window: &mut Window, cx: &mut Context<Self>) {
        use gpui::Focusable as _;
        let between_cards = matches!(
            (&self.focus.focused, &target),
            (Some(Target::Card(a)), Target::Card(b)) if a != b
        );
        self.focus.set(target.clone());
        let handle = match &target {
            Target::Search => self.search.focus_handle(cx),
            Target::Palette => self.palette.focus_handle(cx),
            Target::Field(7) => self.filter.focus_handle(cx),
            Target::Field(n) if *n < 6 => self.numbers[*n as usize].focus_handle(cx),
            other => self.handle(other, cx),
        };
        window.focus(&handle, cx);
        if let Target::Card(i) | Target::Favorite(i) = target {
            self.selected = self.filtered.get(i).copied();
            let row = i / self.grid_model().cols.max(1);
            self.grid_scroll
                .scroll_to_item(row, gpui::ScrollStrategy::Nearest);
        }
        if between_cards {
            self.bench_rendered("dpad");
        }
        cx.notify();
    }

    fn on_move(&mut self, dir: Dir, window: &mut Window, cx: &mut Context<Self>) {
        // Library grid moves come from the grid model, so cards the list culled are reachable.
        if self.route() == Route::Library && self.overlay.is_none() {
            match self.focus.focused.clone() {
                Some(Target::Card(i)) => {
                    if let Some(next) = self.grid_model().step(i, dir) {
                        return self.focus_to(Target::Card(next), window, cx);
                    }
                    if dir == Dir::Up {
                        return self.focus_to(Target::Search, window, cx);
                    }
                    return;
                }
                Some(Target::Search) if dir == Dir::Down && !self.filtered.is_empty() => {
                    return self.focus_to(Target::Card(0), window, cx);
                }
                _ => {}
            }
        }
        let items = self.registry.snapshot();
        if let Some(t) = self.focus.spatial(&items, dir) {
            self.focus_to(t, window, cx);
        }
    }

    fn on_zone(&mut self, forward: bool, window: &mut Window, cx: &mut Context<Self>) {
        let items = self.registry.snapshot();
        if let Some(t) = self.focus.cycle_zone(&items, forward) {
            self.focus_to(t, window, cx);
        }
    }

    /// A / Enter / Space on a spike-owned target. Kit inputs and the select consume their own
    /// Enter first (their key context binds it), so this only sees wrapper targets.
    fn activate(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(target) = self.focus.focused.clone() else {
            return;
        };
        match target {
            Target::Nav(n) => self.navigate(Route::ALL[usize::from(n) % 3], cx),
            Target::Back => self.go_back(cx),
            Target::Forward => self.go_forward(cx),
            Target::Card(i) => self.focus_to(Target::Favorite(i), window, cx),
            Target::Favorite(i) => self.toggle_favorite(i, cx),
            Target::Field(n @ (6 | 8..=12)) => self.set_flag(n, !self.flag(n), cx),
            Target::Field(13) => self.save(window, cx),
            Target::Field(14) => self.open_overlay(Overlay::Modal, window, cx),
            Target::Field(_) | Target::Search | Target::Palette => {}
            Target::InspectorAction(0) => self.focus_to(Target::Card(0), window, cx),
            Target::InspectorAction(_) => self.open_overlay(Overlay::Palette, window, cx),
            Target::Overlay(0) => self.pick_file(window, cx),
            Target::Overlay(_) => self.close_overlay(window, cx),
        }
    }

    fn on_back(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            return self.close_overlay(window, cx);
        }
        self.go_back(cx);
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        self.history.back();
        cx.notify();
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.history.forward();
        cx.notify();
    }

    /// Opens the modal (Kit `Dialog` through `WindowExt::open_dialog`, which focus-traps Tab
    /// and closes on Escape) or the palette (a Kit `Dialog` with an input).
    pub fn open_overlay(&mut self, which: Overlay, window: &mut Window, cx: &mut Context<Self>) {
        self.opener = self.focus.focused.clone();
        self.overlay = Some(which);
        self.focus.overlay_open = true;
        let this = cx.entity();
        let on_close = {
            let this = this.clone();
            move |_: &gpui::ClickEvent, window: &mut Window, cx: &mut gpui::App| {
                this.update(cx, |s, cx| s.overlay_closed(window, cx));
            }
        };
        match which {
            Overlay::Modal => {
                let b0 = self.handle(&Target::Overlay(0), cx);
                let b1 = self.handle(&Target::Overlay(1), cx);
                let reg = self.registry.clone();
                window.open_dialog(cx, move |dialog, window, _| {
                    super::view::modal_body(dialog, &this, &b0, &b1, &reg, window)
                        .on_close(on_close.clone())
                });
                let first = Target::Overlay(0);
                let h = self.handle(&first, cx);
                self.focus.set(first);
                window.focus(&h, cx);
            }
            Overlay::Palette => {
                let palette = self.palette.clone();
                window.open_dialog(cx, move |dialog, _, _| {
                    super::view::palette_body(dialog, &palette).on_close(on_close.clone())
                });
                self.focus_to(Target::Palette, window, cx);
            }
        }
        cx.notify();
    }

    /// Closes the top dialog; focus restore happens in `overlay_closed`.
    pub fn close_overlay(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.is_some() {
            window.close_dialog(cx);
            self.overlay_closed(window, cx);
        }
    }

    /// Dialog closed (Escape, B, scrim, Close button): drop the trap and refocus the opener.
    pub fn overlay_closed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.overlay.take().is_none() {
            return;
        }
        let opener = self.opener.take();
        self.focus.close_overlay(opener.clone());
        if let Some(t) = opener {
            self.focus_to(t, window, cx);
        }
        cx.notify();
    }

    /// Registers the action handlers on the shell root element.
    pub fn bind_actions(
        &self,
        root: gpui::Stateful<gpui::Div>,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        use gpui::InteractiveElement as _;
        root.on_action(cx.listener(|s, _: &DpadUp, w, cx| s.on_move(Dir::Up, w, cx)))
            .on_action(cx.listener(|s, _: &DpadDown, w, cx| s.on_move(Dir::Down, w, cx)))
            .on_action(cx.listener(|s, _: &DpadLeft, w, cx| s.on_move(Dir::Left, w, cx)))
            .on_action(cx.listener(|s, _: &DpadRight, w, cx| s.on_move(Dir::Right, w, cx)))
            .on_action(cx.listener(|s, _: &Confirm, w, cx| s.activate(w, cx)))
            .on_action(cx.listener(|s, _: &Back, w, cx| s.on_back(w, cx)))
            .on_action(cx.listener(|s, _: &NextZone, w, cx| s.on_zone(true, w, cx)))
            .on_action(cx.listener(|s, _: &PrevZone, w, cx| s.on_zone(false, w, cx)))
            .on_action(cx.listener(|s, _: &OpenPalette, w, cx| {
                if s.overlay.is_none() {
                    s.open_overlay(Overlay::Palette, w, cx);
                }
            }))
            .on_action(cx.listener(|s, _: &HistoryBack, _, cx| s.go_back(cx)))
            .on_action(cx.listener(|s, _: &HistoryForward, _, cx| s.go_forward(cx)))
    }

    /// Zone the engine currently has focus in (for the status bar).
    pub fn zone_label(&self) -> &'static str {
        match self.focus.zone() {
            Some(Zone::Sidebar) => "sidebar",
            Some(Zone::Content) => "content",
            Some(Zone::Inspector) => "inspector",
            Some(Zone::Overlay) => "overlay",
            None => "none",
        }
    }
}
