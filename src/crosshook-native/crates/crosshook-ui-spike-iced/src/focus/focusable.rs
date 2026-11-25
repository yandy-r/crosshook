//! `Focusable` wrapper: makes any element (button, checkbox, pick_list, toggler, card) part of
//! the focus graph WITHOUT forking it. It forwards every call to the inner widget, adds an
//! `operation.focusable(..)` report with its own `Id`, and draws a focus ring when the app says
//! it is focused. Activation (A / Enter) is routed by the app from the focused `Target`, so the
//! inner widget keeps its own mouse behaviour unchanged.

use iced::advanced::layout::{self, Layout};
use iced::advanced::overlay;
use iced::advanced::renderer;
use iced::advanced::widget::operation::{self, Operation};
use iced::advanced::widget::{self, Id, Tree, tree};
use iced::advanced::{Shell, Widget};
use iced::mouse;
use iced::{Border, Color, Element, Event, Length, Rectangle, Size, Vector};

use super::target::Target;

pub const RING: Color = Color::from_rgb(0.35, 0.65, 1.0);

#[derive(Default)]
struct State {
    focused: bool,
}

impl operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.focused
    }
    fn focus(&mut self) {
        self.focused = true;
    }
    fn unfocus(&mut self) {
        self.focused = false;
    }
}

pub struct Focus<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    id: Id,
    focused: bool,
}

pub fn focusable<'a, Message, Theme, Renderer>(
    target: &Target,
    focused: bool,
    content: impl Widget<Message, Theme, Renderer> + 'a,
) -> Focus<'a, Message, Theme, Renderer> {
    Focus {
        content: Widget::_boxed(content),
        id: target.id(),
        focused,
    }
}

impl<Message, Theme, Renderer> widget::Meta for Focus<'_, Message, Theme, Renderer> {}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Focus<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }
    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }
    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }
    fn size(&self) -> Size<Length> {
        self.content.size()
    }
    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.content.layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();
        state.focused = self.focused;
        operation.focusable(Some(&self.id), layout.bounds(), state);
        let (child_layout, child) = layout.iter_mut(&mut tree.children).next().expect("child");
        self.content
            .operate(child, child_layout, viewport, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let (child_layout, child) = layout.iter_mut(&mut tree.children).next().expect("child");
        self.content.update(
            child,
            event,
            child_layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let (child_layout, child) = layout.iter(&tree.children).next().expect("child");
        self.content
            .mouse_interaction(child, child_layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let (child_layout, child) = layout.iter(&tree.children).next().expect("child");
        self.content.draw(
            child,
            renderer,
            theme,
            style,
            child_layout,
            cursor,
            viewport,
        );
        if self.focused {
            renderer.fill_quad(
                renderer::Quad {
                    bounds: layout.bounds().expand(2.0),
                    border: Border {
                        color: RING,
                        width: 2.0,
                        radius: 6.0.into(),
                    },
                    ..renderer::Quad::default()
                },
                Color::TRANSPARENT,
            );
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        let (child_layout, child) = layout.iter_mut(&mut tree.children).next().expect("child");
        self.content
            .overlay(child, child_layout, renderer, viewport, translation, window)
    }
}
