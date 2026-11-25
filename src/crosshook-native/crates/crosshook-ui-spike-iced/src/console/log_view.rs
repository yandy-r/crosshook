//! Culled log view: a custom widget that draws only the lines inside its bounds.
//!
//! Upstream iced has no virtualized list at this revision, so this widget owns its own scroll
//! offset (wheel events) and calls `fill_text` for visible lines only. It is not a fork of any
//! iced widget; it reads the ring buffer by reference.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::text;
use iced::advanced::widget::{self, Tree, tree};
use iced::advanced::{Shell, Widget};
use iced::alignment;
use iced::mouse;
use iced::{Color, Event, Font, Length, Pixels, Point, Rectangle, Size};

use super::ring::Ring;

pub const LINE_HEIGHT: f32 = 18.0;
pub const FOLLOW_THRESHOLD: f32 = 24.0;
const TEXT_SIZE: f32 = 13.0;

/// Scroll state lives in the app (so `follow` and the bench probe can read it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LogScroll {
    /// Distance from the bottom in pixels; 0 = following the tail.
    pub from_bottom: f32,
}

impl Default for LogScroll {
    fn default() -> Self {
        Self { from_bottom: 0.0 }
    }
}

impl LogScroll {
    pub fn following(&self) -> bool {
        self.from_bottom <= FOLLOW_THRESHOLD
    }

    /// Applies a wheel delta (positive y = up) clamped to the content height.
    pub fn scrolled(self, dy_px: f32, content_h: f32, view_h: f32) -> Self {
        let max = (content_h - view_h).max(0.0);
        Self {
            from_bottom: (self.from_bottom + dy_px).clamp(0.0, max),
        }
    }
}

/// First visible line index and count for a viewport.
pub fn visible_range(len: usize, view_h: f32, scroll: LogScroll) -> (usize, usize) {
    let content_h = len as f32 * LINE_HEIGHT;
    let top_px = (content_h - view_h - scroll.from_bottom).max(0.0);
    let first = (top_px / LINE_HEIGHT).floor() as usize;
    let count = (view_h / LINE_HEIGHT).ceil() as usize + 1;
    (first.min(len), count.min(len.saturating_sub(first)))
}

pub struct LogView<'a, Message> {
    ring: &'a Ring,
    scroll: LogScroll,
    on_scroll: Box<dyn Fn(LogScroll) -> Message + 'a>,
}

pub fn log_view<'a, Message>(
    ring: &'a Ring,
    scroll: LogScroll,
    on_scroll: impl Fn(LogScroll) -> Message + 'a,
) -> LogView<'a, Message> {
    LogView {
        ring,
        scroll,
        on_scroll: Box::new(on_scroll),
    }
}

impl<Message> widget::Meta for LogView<'_, Message> {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer> for LogView<'_, Message>
where
    Renderer: text::Renderer,
{
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn layout(&mut self, tree: &mut Tree, _renderer: &Renderer, limits: &layout::Limits) {
        tree.size = limits.max;
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return;
        };
        if !cursor.is_over(layout.bounds()) {
            return;
        }
        let dy = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y * LINE_HEIGHT * 3.0,
            mouse::ScrollDelta::Pixels { y, .. } => *y,
        };
        let view_h = layout.bounds().height;
        let next = self
            .scroll
            .scrolled(dy, self.ring.len() as f32 * LINE_HEIGHT, view_h);
        if next != self.scroll {
            shell.publish((self.on_scroll)(next));
        }
        shell.capture_event();
    }

    fn draw(
        &self,
        _tree: &Tree,
        renderer: &mut Renderer,
        _theme: &Theme,
        _style: &renderer::Style,
        layout: Layout,
        _cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let bounds = layout.bounds();
        renderer.fill_quad(
            renderer::Quad {
                bounds,
                ..renderer::Quad::default()
            },
            Color::from_rgb8(0x0b, 0x0e, 0x14),
        );
        let clip = bounds.intersection(viewport).unwrap_or(bounds);
        let (first, count) = visible_range(self.ring.len(), bounds.height, self.scroll);
        let content_h = self.ring.len() as f32 * LINE_HEIGHT;
        let top_px = (content_h - bounds.height - self.scroll.from_bottom).max(0.0);
        for i in first..first + count {
            let Some(line) = self.ring.get(i) else { break };
            let y = bounds.y + i as f32 * LINE_HEIGHT - top_px;
            renderer.fill_text(
                text::Text {
                    content: line.to_owned(),
                    bounds: Size::new(bounds.width - 12.0, LINE_HEIGHT),
                    size: Pixels(TEXT_SIZE),
                    line_height: text::LineHeight::Absolute(Pixels(LINE_HEIGHT)),
                    font: Font::MONOSPACE,
                    align_x: text::Alignment::Left,
                    align_y: alignment::Vertical::Top,
                    shaping: text::Shaping::Basic,
                    wrapping: text::Wrapping::None,
                    ellipsis: text::Ellipsis::None,
                    hint_factor: None,
                },
                Point::new(bounds.x + 6.0, y),
                Color::from_rgb8(0xc8, 0xd0, 0xdc),
                clip,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn follow_shows_tail() {
        let (first, count) = visible_range(1000, 180.0, LogScroll::default());
        assert_eq!(first, 990);
        assert!(count <= 11);
    }

    #[test]
    fn scroll_clamps_and_unfollows() {
        let s = LogScroll::default().scrolled(500.0, 18_000.0, 180.0);
        assert!(!s.following());
        let s = s.scrolled(1e9, 18_000.0, 180.0);
        assert_eq!(s.from_bottom, 18_000.0 - 180.0);
        assert!(s.scrolled(-1e9, 18_000.0, 180.0).following());
    }
}
