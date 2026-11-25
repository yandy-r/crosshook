//! Focus routing: collect targets from the live tree, ask the pure engine, then apply.
//! Text inputs get iced focus via `operation::focus`; other targets are app-owned (the wrapper
//! draws the ring) and iced text focus is cleared so typing does not leak into a field.

use iced::Task;
use iced::advanced::widget::operate;
use iced::widget::operation;

use super::{App, Message, Pending, Route};
use crate::focus::collect::Collect;
use crate::focus::{Dir, Target};
use crate::gamepad::Nav;
use crate::library::grid;

impl App {
    pub fn collect(&self, pending: Pending) -> Task<Message> {
        operate(Collect::default()).map(move |items| Message::Collected(items, pending.clone()))
    }

    pub fn on_nav(&mut self, nav: Nav) -> Task<Message> {
        match nav {
            Nav::Confirm => self.activate(),
            Nav::Back => self.on_escape(),
            Nav::Move(dir) if self.route() == Route::Library && !self.focus.overlay_open => {
                // Grid moves are computed from the grid model, so off-screen (culled) cards
                // are reachable too; the scroll follows the focused card.
                if let Some(Target::Card(i)) = self.focus.focused.clone() {
                    let g = grid::Grid::new(self.view_w, self.filtered.len());
                    if let Some(next) = g.step(i, dir) {
                        return self.focus_to(Target::Card(next));
                    }
                    if dir == Dir::Up {
                        return self.focus_to(Target::Search);
                    }
                    return Task::none();
                }
                if self.focus.focused == Some(Target::Search)
                    && dir == Dir::Down
                    && !self.filtered.is_empty()
                {
                    return self.focus_to(Target::Card(0));
                }
                self.collect(Pending::Nav(nav))
            }
            _ => self.collect(Pending::Nav(nav)),
        }
    }

    pub fn on_collected(
        &mut self,
        items: &[(Target, iced::Rectangle)],
        pending: Pending,
    ) -> Task<Message> {
        let next = match pending {
            Pending::Tab(forward) => self.focus.linear(items, forward),
            Pending::Nav(Nav::Move(dir)) => self.focus.spatial(items, dir),
            Pending::Nav(Nav::NextZone) => self.focus.cycle_zone(items, true),
            Pending::Nav(Nav::PrevZone) => self.focus.cycle_zone(items, false),
            Pending::Nav(_) => None,
        };
        next.map_or_else(Task::none, |t| self.focus_to(t))
    }

    pub fn focus_to(&mut self, target: Target) -> Task<Message> {
        let moved_between_cards = matches!(
            (&self.focus.focused, &target),
            (Some(Target::Card(a)), Target::Card(b)) if a != b
        );
        self.focus.set(target.clone());
        let mut tasks = Vec::new();
        if target.is_text_input() {
            tasks.push(operation::focus(target.id()));
        } else {
            tasks.push(operation::focus(iced::widget::Id::new("__none__")));
        }
        if let Target::Card(i) = target {
            self.selected = self.filtered.get(i).copied();
            let g = grid::Grid::new(self.view_w, self.filtered.len());
            if let Some(y) = g.scroll_to_reveal(i, self.scroll_y, self.view_h) {
                tasks.push(operation::scrollable::scroll_to(
                    super::scroll_id(Route::Library),
                    operation::scrollable::AbsoluteOffset {
                        x: None,
                        y: Some(y),
                    },
                    operation::Animation::Instant,
                ));
            }
        }
        if moved_between_cards {
            tasks.push(self.bench_rendered("dpad"));
        }
        Task::batch(tasks)
    }

    pub fn on_escape(&mut self) -> Task<Message> {
        if self.overlay.is_some() {
            return self.close_overlay();
        }
        if self.history.can_back() {
            self.history.back();
        }
        Task::none()
    }

    /// A / Enter on the focused app-owned target.
    pub fn activate(&mut self) -> Task<Message> {
        let Some(target) = self.focus.focused.clone() else {
            return Task::none();
        };
        match target {
            Target::Nav(n) => self.update(Message::Navigate(Route::ALL[usize::from(n) % 3])),
            Target::Back => self.update(Message::Back),
            Target::Forward => self.update(Message::Forward),
            Target::Card(i) => self.focus_to(Target::Favorite(i)),
            Target::Favorite(i) => self.update(Message::ToggleFavorite(i)),
            Target::Field(6) => self.update(Message::Enabled(!self.draft.base.enabled)),
            Target::Field(7) => {
                // pick_list has no keyboard API at this rev: A cycles the value instead.
                let pos = crate::form::FILTERS
                    .iter()
                    .position(|f| Some(&f.0) == self.draft.base.upscale_filter.as_ref())
                    .map_or(0, |p| (p + 1) % crate::form::FILTERS.len());
                self.update(Message::Filter(crate::form::FILTERS[pos].clone()))
            }
            Target::Field(n @ 8..=11) => {
                let b = &self.draft.base;
                let on = match n {
                    8 => b.fullscreen,
                    9 => b.borderless,
                    10 => b.grab_cursor,
                    _ => b.hdr_enabled,
                };
                self.update(Message::Flag(n, !on))
            }
            Target::Field(12) => self.update(Message::Optimization(!self.draft.optimization)),
            Target::Field(13) => self.update(Message::Save),
            Target::Field(14) => self.update(Message::OpenModal),
            Target::Field(_) | Target::Search | Target::Palette => Task::none(),
            Target::InspectorAction(0) => self.focus_to(Target::Card(0)),
            Target::InspectorAction(_) => self.update(Message::OpenPalette),
            Target::Overlay(0) => self.update(Message::PickFile),
            Target::Overlay(_) => self.update(Message::CloseModal),
        }
    }
}
