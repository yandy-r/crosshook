//! Library: a GPUI Kit `v_virtual_list` of grid rows. The list lays out only the visible rows
//! (+ its own overscan) and reports the range, which drives async cover decode.

use std::rc::Rc;

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement, ParentElement as _,
    StatefulInteractiveElement as _, Styled as _, Window, canvas, div, img,
    prelude::FluentBuilder as _, px, rgb, size,
};
use gpui_component::input::Input;
use gpui_component::{IconName, h_flex, v_flex, v_virtual_list};

use super::Shell;
use crate::focus::{Target, wrap};
use crate::library::grid::{CARD_W, GAP, Grid, PAD};

impl Shell {
    pub fn library(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let search = wrap(
            Target::Search,
            None,
            &self.registry,
            window,
            Input::new(&self.search).w(px(320.)),
        );
        let header = h_flex()
            .gap_2()
            .p(px(PAD))
            .child(IconName::Search)
            .child(search)
            .child(div().text_sm().child(self.status.clone()));
        let rows = self.grid_model().rows();
        if self.row_sizes.len() != rows {
            self.row_sizes = Rc::new(vec![size(px(1.), px(Grid::row_h())); rows]);
        }
        let this = cx.entity();
        // Columns follow the list's width, measured each frame by a zero-height probe.
        let width_probe = canvas(
            move |bounds, _, cx| {
                let cols = Grid::new(bounds.size.width.as_f32(), 1).cols;
                this.update(cx, |s, cx| {
                    if s.cols != cols {
                        s.cols = cols;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .w_full()
        .h(px(0.));
        let list = v_virtual_list(
            cx.entity(),
            "library-grid",
            self.row_sizes.clone(),
            |shell, range, window, cx| {
                shell.want_rows(range.clone(), cx);
                range
                    .map(|row| shell.grid_row(row, window, cx))
                    .collect::<Vec<_>>()
            },
        )
        .track_scroll(&self.grid_scroll);
        v_flex()
            .size_full()
            .child(header)
            .child(width_probe)
            .child(div().flex_1().min_h_0().px(px(PAD)).child(list))
            .into_any_element()
    }

    fn grid_row(&mut self, row: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let cols = self.cols.max(1);
        let mut r = h_flex().gap(px(GAP)).h(px(Grid::row_h())).items_start();
        for c in 0..cols {
            let display = row * cols + c;
            if display >= self.filtered.len() {
                break;
            }
            r = r.child(self.card(display, window, cx));
        }
        r.into_any_element()
    }

    fn card(&mut self, display: usize, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let idx = self.filtered[display];
        let summary = &self.summaries[idx];
        let title = summary.title.clone();
        let cover = self.covers.get(&summary.app_id).cloned();
        let fav = self.favorites.contains(&idx);
        let card_t = Target::Card(display);
        let fav_t = Target::Favorite(display);
        let card_h = self.handle(&card_t, cx);
        let fav_h = self.handle(&fav_t, cx);
        let art = match cover {
            Some(d) => div()
                .bg(gpui::rgb(u32::from_be_bytes([
                    0,
                    d.dominant[0],
                    d.dominant[1],
                    d.dominant[2],
                ])))
                .child(img(d.image.clone()).w(px(CARD_W)).h(px(CARD_W * 1.5)))
                .into_any_element(),
            None => div()
                .w(px(CARD_W))
                .h(px(CARD_W * 1.5))
                .bg(rgb(0x222a36))
                .into_any_element(),
        };
        let tint = self.covers.peek_dominant(&self.summaries[idx].app_id);
        let art = match tint {
            Some([r, g, b]) if !self.covers.contains(&self.summaries[idx].app_id) => div()
                .w(px(CARD_W))
                .h(px(CARD_W * 1.5))
                .bg(rgb(u32::from_be_bytes([0, r, g, b])))
                .into_any_element(),
            _ => art,
        };
        let star = div()
            .id(("fav", display))
            .p_1()
            .child(if fav {
                IconName::Star
            } else {
                IconName::StarOff
            })
            .on_click(cx.listener(move |s, _, _, cx| s.toggle_favorite(display, cx)));
        let fav_el = wrap(fav_t, Some(&fav_h), &self.registry, window, star);
        let body = v_flex()
            .id(("card", display))
            .w(px(CARD_W))
            .gap_1()
            .child(art)
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .truncate()
                            .max_w(px(CARD_W - 36.))
                            .child(title),
                    )
                    .child(fav_el),
            )
            .on_click(cx.listener(move |s, _, window, cx| {
                s.focus_to(Target::Card(display), window, cx);
            }))
            .when(self.selected == Some(idx), |d| d.bg(rgb(0x1b2533)));
        wrap(card_t, Some(&card_h), &self.registry, window, body)
    }
}
