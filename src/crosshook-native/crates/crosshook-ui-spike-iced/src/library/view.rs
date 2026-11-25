//! Culled library grid: a spacer for the rows above, widgets for visible rows only, a spacer
//! for the rows below. The scrollable's content height therefore stays exact for 500 cards
//! while only ~20-30 cards exist in the widget tree.

use iced::widget::{
    Column, Row, button, column, container, image, row, scrollable, space, text, text_input,
};
use iced::{Background, Border, Color, Element, Fill, Widget};

use super::grid::{CARD_H, CARD_W, GAP, Grid, HEADER_H, PAD};
use crate::app::{App, LIBRARY_SCROLL, Message};
use crate::focus::{Target, focusable};
use crate::icons::{Icon, icon};

pub fn view(app: &App) -> Element<'_, Message> {
    let g = Grid::new(app.view_w, app.filtered.len());
    let (first_row, last_row) = g.visible_rows(app.scroll_y, app.view_h);

    let search = focusable(
        &Target::Search,
        app.focused_is(&Target::Search),
        text_input("Search games", &app.query)
            .id(Target::Search.id())
            .on_input(Message::Search)
            .padding(8)
            .width(320),
    );
    let header = row![icon(Icon::Search, 18.0), search, text(&app.status).size(13)]
        .spacing(10)
        .align_y(iced::Center)
        .height(HEADER_H);

    let mut rows = Column::new().spacing(GAP);
    for r in first_row..last_row {
        let mut cards = Row::new().spacing(GAP);
        for c in 0..g.cols {
            let display = r * g.cols + c;
            if display >= app.filtered.len() {
                break;
            }
            cards = cards.push(card(app, display));
        }
        rows = rows.push(cards);
    }
    let above = first_row as f32 * Grid::row_h();
    let below = g.rows().saturating_sub(last_row) as f32 * Grid::row_h();
    let content = column![header, space().height(above), rows, space().height(below)].padding(PAD);

    scrollable(content)
        .id(iced::widget::Id::new(LIBRARY_SCROLL))
        .on_scroll(|s| Message::LibraryScrolled(s.viewport))
        .width(Fill)
        .height(Fill)
        .boxed()
}

fn card(app: &App, display: usize) -> Element<'_, Message> {
    let idx = app.filtered[display];
    let summary = &app.summaries[idx];
    let cover = app.covers.peek(&summary.app_id);
    let tint = cover.map_or(Color::from_rgb8(0x22, 0x27, 0x30), |d| {
        let [r, g, b] = d.dominant;
        Color::from_rgb8(r / 3, g / 3, b / 3)
    });
    let art: Element<'_, Message> = match cover {
        Some(decoded) => image(decoded.handle.clone())
            .width(CARD_W)
            .height(CARD_W * 1.5)
            .boxed(),
        None => container(space())
            .width(CARD_W)
            .height(CARD_W * 1.5)
            .boxed(),
    };
    let fav = app.favorites.contains(&idx);
    let fav_target = Target::Favorite(display);
    let fav_button = focusable(
        &fav_target,
        app.focused_is(&fav_target),
        button(icon(if fav { Icon::Star } else { Icon::StarOff }, 16.0))
            .on_press(Message::ToggleFavorite(display))
            .padding(4),
    );
    let body = column![
        art,
        row![text(&summary.title).size(13).width(Fill), fav_button]
            .align_y(iced::Center)
            .padding(6),
    ];
    let card_target = Target::Card(display);
    let selected = app.selected == Some(idx);
    let inner = button(body)
        .padding(0)
        .on_press(Message::Select(display))
        .style(move |_, _| button::Style {
            background: Some(Background::Color(tint)),
            border: Border {
                color: if selected {
                    Color::from_rgb(0.9, 0.7, 0.2)
                } else {
                    Color::TRANSPARENT
                },
                width: 1.0,
                radius: 6.0.into(),
            },
            text_color: Color::WHITE,
            ..button::Style::default()
        })
        .width(CARD_W)
        .height(CARD_H);
    focusable(&card_target, app.focused_is(&card_target), inner).boxed()
}
