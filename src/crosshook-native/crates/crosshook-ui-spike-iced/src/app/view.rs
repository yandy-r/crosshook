//! Shell layout: sidebar | content | inspector, console docked below in the wide layout,
//! overlays (modal, palette) stacked on top behind an opaque scrim.

use iced::widget::{
    Column, button, center, column, container, mouse_area, opaque, row, space, stack, text,
    text_input,
};
use iced::{Background, Color, Element, Fill, Widget};

use super::{App, CONSOLE_H, INSPECTOR_W, Message, Overlay, Route, SIDEBAR_W};
use crate::console::log_view;
use crate::focus::{Target, focusable};
use crate::icons::{Icon, icon};

impl App {
    pub fn view(&self) -> Element<'_, Message> {
        let body = row![self.sidebar(), self.content(), self.inspector()].height(Fill);
        let shell: Element<'_, Message> = if self.wide() {
            column![body, self.console_panel()].boxed()
        } else {
            column![body, self.status_bar()].boxed()
        };
        match &self.overlay {
            None => shell,
            Some(Overlay::Modal { .. }) => stack![shell, scrim(self.modal())].boxed(),
            Some(Overlay::Palette { .. }) => stack![shell, scrim(self.palette_view())].boxed(),
        }
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let icons = [Icon::Library, Icon::Settings, Icon::Terminal];
        let mut nav: Column<Element<'_, Message>> =
            Column::new().spacing(6).padding(10).width(SIDEBAR_W);
        nav = nav.push(text("CrossHook").size(18).boxed());
        for (i, route) in Route::ALL.into_iter().enumerate() {
            let target = Target::Nav(i as u8);
            let active = self.route() == route;
            let label = row![icon(icons[i], 16.0), text(route.label())].spacing(8);
            let b = button(label)
                .width(Fill)
                .on_press(Message::Navigate(route))
                .style(if active {
                    button::primary
                } else {
                    button::text
                });
            nav = nav.push(focusable(&target, self.focused_is(&target), b).boxed());
        }
        let back =
            button(text("◀ Back")).on_press_maybe(self.history.can_back().then_some(Message::Back));
        let fwd = button(text("Fwd ▶"))
            .on_press_maybe(self.history.can_forward().then_some(Message::Forward));
        nav = nav.push(space().height(20).boxed()).push(
            row![
                focusable(&Target::Back, self.focused_is(&Target::Back), back),
                focusable(&Target::Forward, self.focused_is(&Target::Forward), fwd),
            ]
            .spacing(6)
            .boxed(),
        );
        container(nav).height(Fill).style(panel).boxed()
    }

    fn content(&self) -> Element<'_, Message> {
        let page: Element<'_, Message> = match self.route() {
            Route::Library => crate::library::view::view(self),
            Route::Settings => crate::settings_view::view(self),
            Route::Console => container(
                log_view(
                    &self.console.ring,
                    self.console.scroll,
                    Message::ConsoleScrolled,
                )
                .boxed(),
            )
            .padding(8)
            .boxed(),
        };
        container(page).width(Fill).height(Fill).boxed()
    }

    fn inspector(&self) -> Element<'_, Message> {
        let selected = self.selected.and_then(|i| self.summaries.get(i));
        let title = selected.map_or("Nothing selected", |s| s.title.as_str());
        let app_id = selected.map_or(String::new(), |s| format!("App ID {}", s.app_id));
        let a0 = Target::InspectorAction(0);
        let a1 = Target::InspectorAction(1);
        let col = column![
            text("Inspector").size(16),
            text(title),
            text(app_id).size(12),
            focusable(
                &a0,
                self.focused_is(&a0),
                button(text("Back to grid")).on_press(Message::FocusTarget(Target::Card(0)))
            ),
            focusable(
                &a1,
                self.focused_is(&a1),
                button(text("Command palette")).on_press(Message::OpenPalette)
            ),
            text(format!(
                "Covers cached: {} ({:.1} MiB, {} evicted)",
                self.covers.len(),
                self.covers.bytes() as f64 / 1048576.0,
                self.covers.evictions
            ))
            .size(11),
            text(&self.renderer).size(11),
        ]
        .spacing(10)
        .padding(10)
        .width(INSPECTOR_W);
        container(col).height(Fill).style(panel).boxed()
    }

    fn console_panel(&self) -> Element<'_, Message> {
        let header = row![
            icon(Icon::Terminal, 14.0),
            text(format!(
                "Console — {} lines, {} dropped{}",
                self.console.ring.len(),
                self.console.ring.dropped(),
                if self.console.scroll.following() {
                    ", following"
                } else {
                    ""
                }
            ))
            .size(12),
        ]
        .spacing(6);
        container(
            column![
                header,
                log_view(
                    &self.console.ring,
                    self.console.scroll,
                    Message::ConsoleScrolled
                )
                .boxed(),
            ]
            .spacing(4),
        )
        .padding(6)
        .height(CONSOLE_H)
        .width(Fill)
        .style(panel)
        .boxed()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        container(
            text(format!(
                "{} · console {} lines",
                self.status,
                self.console.ring.len()
            ))
            .size(12),
        )
        .padding(4)
        .width(Fill)
        .style(panel)
        .boxed()
    }

    fn modal(&self) -> Element<'_, Message> {
        let o0 = Target::Overlay(0);
        let o1 = Target::Overlay(1);
        container(
            column![
                text("Pick game executable").size(18),
                text("Opens the xdg-desktop-portal file chooser (no GTK).").size(12),
                row![
                    focusable(
                        &o0,
                        self.focused_is(&o0),
                        button(row![icon(Icon::Folder, 16.0), text("Browse…")].spacing(6))
                            .on_press(Message::PickFile)
                    ),
                    focusable(
                        &o1,
                        self.focused_is(&o1),
                        button(row![icon(Icon::Close, 16.0), text("Close")].spacing(6))
                            .on_press(Message::CloseModal)
                    ),
                ]
                .spacing(10),
            ]
            .spacing(12),
        )
        .padding(20)
        .width(420)
        .style(container::rounded_box)
        .boxed()
    }

    fn palette_view(&self) -> Element<'_, Message> {
        container(
            column![
                text("Go to…").size(14),
                focusable(
                    &Target::Palette,
                    self.focused_is(&Target::Palette),
                    text_input("library, settings, console", &self.palette)
                        .id(Target::Palette.id())
                        .on_input(Message::Palette)
                        .on_submit(Message::PaletteSubmit)
                        .padding(8)
                ),
            ]
            .spacing(8),
        )
        .padding(16)
        .width(420)
        .style(container::rounded_box)
        .boxed()
    }
}

fn scrim<'a>(content: Element<'a, Message>) -> Element<'a, Message> {
    opaque(
        mouse_area(center(opaque(content)).style(|_| container::Style {
            background: Some(Background::Color(Color {
                a: 0.7,
                ..Color::BLACK
            })),
            ..container::Style::default()
        }))
        .on_press(Message::CloseModal),
    )
    .boxed()
}

fn panel(_: &iced::Theme) -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb8(0x14, 0x18, 0x20))),
        ..container::Style::default()
    }
}
