//! Culled console view: `uniform_list` lays out only the visible rows of the 10k ring.
//! Follow mode scrolls to the tail on every batch until the user scrolls up.

use gpui::{
    Context, InteractiveElement, IntoElement, ParentElement, Render, ScrollWheelEvent,
    SharedString, Styled, UniformListScrollHandle, Window, div, px, rgb, uniform_list,
};

use super::Console;

pub const LINE_HEIGHT: f32 = 16.0;

pub struct ConsoleView {
    pub console: Console,
    pub scroll: UniformListScrollHandle,
}

impl ConsoleView {
    pub fn new() -> Self {
        Self {
            console: Console::new(),
            scroll: UniformListScrollHandle::new(),
        }
    }

    pub fn append(&mut self, batch: Vec<String>, cx: &mut Context<Self>) {
        self.console.append(batch);
        if self.console.follow {
            self.scroll.scroll_to_bottom();
        }
        cx.notify();
    }

    fn on_wheel(&mut self, ev: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let dy = ev.delta.pixel_delta(px(LINE_HEIGHT)).y;
        if dy > px(0.) {
            self.console.follow = false;
        } else if self.scroll.is_scrolled_to_end().unwrap_or(true) {
            self.console.follow = true;
        }
        cx.notify();
    }

    pub fn header(&self) -> SharedString {
        let ring = &self.console.ring;
        format!(
            "Console — {} lines, {} dropped{}",
            ring.len(),
            ring.dropped(),
            if self.console.follow {
                ", following"
            } else {
                ""
            }
        )
        .into()
    }
}

impl Render for ConsoleView {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let count = self.console.ring.len();
        div()
            .id("console")
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(0x0d1016))
            .child(
                div()
                    .px_2()
                    .text_xs()
                    .text_color(rgb(0x9aa4b2))
                    .child(self.header()),
            )
            .child(
                uniform_list("console-lines", count, move |range, _, cx| {
                    let view = entity.read(cx);
                    range
                        .filter_map(|i| view.console.ring.get(i))
                        .map(|line| {
                            div()
                                .h(px(LINE_HEIGHT))
                                .px_2()
                                .text_xs()
                                .font_family("monospace")
                                .text_color(rgb(0xc8d0da))
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .child(SharedString::from(line.to_owned()))
                        })
                        .collect()
                })
                .track_scroll(&self.scroll)
                .on_scroll_wheel(cx.listener(Self::on_wheel))
                .flex_1(),
            )
    }
}
