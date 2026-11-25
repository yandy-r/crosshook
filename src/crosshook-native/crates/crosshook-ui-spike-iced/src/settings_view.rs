//! Settings page: the Gamescope form on the real core type, inside a scrollable that is taller
//! than the window (so the G4 `settings` scenario has something to scroll).

use iced::widget::{
    Column, button, checkbox, pick_list, row, scrollable, space, text, text_input, toggler,
};
use iced::{Color, Element, Fill, Widget};

use crate::app::{App, Message, SETTINGS_SCROLL};
use crate::focus::{Target, focusable};
use crate::form::{FILTERS, Filter, NUMERIC};

fn wrap<'a>(app: &App, n: u8, el: Element<'a, Message>) -> Element<'a, Message> {
    let t = Target::Field(n);
    focusable(&t, app.focused_is(&t), el).boxed()
}

pub fn view(app: &App) -> Element<'_, Message> {
    let d = &app.draft;
    let errors = d.errors();
    let f = |n: u8| Target::Field(n);

    let mut col: Column<Element<'_, Message>> = Column::new().spacing(14).padding(20).width(720);
    col = col.push(text("Settings — Gamescope").size(22).boxed());
    col = col.push(
        text(format!(
            "Profile: {}",
            app.form_profile.as_deref().unwrap_or("(none)")
        ))
        .size(12)
        .boxed(),
    );
    col = col.push(
        wrap(
            app,
            6,
            toggler(d.base.enabled)
                .label("Enable gamescope")
                .on_toggle(Message::Enabled)
                .boxed(),
        )
        .boxed(),
    );

    for (i, label) in NUMERIC.iter().enumerate() {
        let input = text_input("auto", &d.numbers[i])
            .id(f(i as u8).id())
            .on_input(move |v| Message::Number(i, v))
            .padding(6)
            .width(160);
        let mut line: Column<Element<'_, Message>> = Column::new().push(
            row![text(*label).width(180), wrap(app, i as u8, input.boxed())]
                .align_y(iced::Center)
                .boxed(),
        );
        if let Some(err) = &errors[i] {
            line = line.push(
                text(err.clone())
                    .size(12)
                    .color(Color::from_rgb(1.0, 0.45, 0.45))
                    .boxed(),
            );
        }
        col = col.push(line.boxed());
    }

    let selected = d.base.upscale_filter.clone().map(Filter);
    col = col.push(
        row![
            text("Upscale filter").width(180),
            wrap(
                app,
                7,
                pick_list(selected, FILTERS.as_slice(), Filter::to_string)
                    .on_select(Message::Filter)
                    .boxed()
            ),
        ]
        .align_y(iced::Center)
        .boxed(),
    );

    let flags = [
        (8u8, "Fullscreen", d.base.fullscreen),
        (9, "Borderless", d.base.borderless),
        (10, "Grab cursor", d.base.grab_cursor),
        (11, "HDR", d.base.hdr_enabled),
    ];
    for (n, label, on) in flags {
        col = col.push(
            wrap(
                app,
                n,
                checkbox(on)
                    .label(label)
                    .on_toggle(move |v| Message::Flag(n, v))
                    .boxed(),
            )
            .boxed(),
        );
    }
    col = col.push(text("Launch optimizations").size(16).boxed());
    col = col.push(
        wrap(
            app,
            12,
            checkbox(d.optimization)
                .label("Disable Steam overlay (optimization)")
                .on_toggle(Message::Optimization)
                .boxed(),
        )
        .boxed(),
    );
    col = col.push(
        row![
            wrap(
                app,
                13,
                button(text("Save")).on_press(Message::Save).boxed()
            ),
            wrap(
                app,
                14,
                button(text("Pick file…"))
                    .on_press(Message::OpenModal)
                    .boxed()
            ),
        ]
        .spacing(10)
        .boxed(),
    );
    // Extra height so the page scrolls (G4 `settings`).
    for i in 0..30 {
        col = col.push(
            text(format!("Advanced option placeholder {i}"))
                .size(13)
                .boxed(),
        );
    }
    col = col.push(space().height(200).boxed());

    scrollable(col)
        .id(iced::widget::Id::new(SETTINGS_SCROLL))
        .width(Fill)
        .height(Fill)
        .boxed()
}
