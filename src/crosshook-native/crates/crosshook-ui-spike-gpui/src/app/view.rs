//! Shell layout: sidebar | content | inspector, console docked below in the wide layout.
//! Overlays are GPUI Kit dialogs (rendered by Kit's Root dialog layer, not here).

use gpui::{
    AnyElement, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement as _, Render, Styled as _, Window, div, prelude::FluentBuilder as _, px, rgb,
};
use gpui_component::IconName;
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::dialog::Dialog;
use gpui_component::input::{Input, InputState};
use gpui_component::{h_flex, v_flex};

use super::{CONSOLE_H, INSPECTOR_W, Route, SIDEBAR_W, Shell};
use crate::focus::{Registry, Target, wrap};

pub const PANEL: u32 = 0x141820;

impl Render for Shell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.registry.clear();
        self.sync_focus_from_window(window, cx);
        let body = h_flex()
            .flex_1()
            .min_h_0()
            .child(self.sidebar(window, cx))
            .child(self.content(window, cx))
            .child(self.inspector(window, cx));
        let bottom = if self.wide(window) {
            div()
                .h(px(CONSOLE_H))
                .flex_shrink_0()
                .child(self.console.clone())
                .into_any_element()
        } else {
            self.status_bar(cx)
        };
        self.bench_after_render(window, cx);
        // With nothing focused, gpui dispatches key actions at the window root (Kit's Root),
        // above this subtree, so the shell root owns a focus handle and takes focus when
        // nothing else has it: F6 / LB / RB / D-pad then work from a cold start.
        if window.focused(cx).is_none() {
            window.focus(&self.root_focus, cx);
        }
        let root = v_flex()
            .id("shell")
            .key_context("Shell")
            .track_focus(&self.root_focus)
            .size_full()
            .bg(rgb(0x0b0e13))
            .text_color(rgb(0xe6e9ee))
            .child(body)
            .child(bottom);
        super::nav::nav_actions(root, &cx.entity())
    }
}

impl Shell {
    fn button(
        &mut self,
        target: Target,
        label: &str,
        icon: Option<IconName>,
        active: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let handle = self.handle(&target, cx);
        let t = target.clone();
        let b = Button::new(format!("btn-{target:?}"))
            .label(label.to_owned())
            .when_some(icon, |b, i| b.icon(i))
            .when(active, |b| b.primary())
            .when(!active, |b| b.ghost())
            .tab_stop(false)
            .on_click(cx.listener(move |this, _, window, cx| {
                this.focus_to(t.clone(), window, cx);
                window.dispatch_action(Box::new(crate::actions::Confirm), cx);
            }));
        wrap(target, Some(&handle), &self.registry, window, b)
    }

    fn sidebar(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let icons = [
            IconName::BookOpen,
            IconName::Settings,
            IconName::SquareTerminal,
        ];
        let mut nav = v_flex()
            .w(px(SIDEBAR_W))
            .flex_shrink_0()
            .p_2()
            .gap_1()
            .bg(rgb(PANEL))
            .child(div().text_lg().child("CrossHook"));
        for (i, route) in Route::ALL.into_iter().enumerate() {
            let active = self.route() == route;
            nav = nav.child(self.button(
                Target::Nav(i as u8),
                route.label(),
                Some(icons[i].clone()),
                active,
                window,
                cx,
            ));
        }
        let back = self.button(
            Target::Back,
            "Back",
            Some(IconName::ArrowLeft),
            false,
            window,
            cx,
        );
        let fwd = self.button(
            Target::Forward,
            "Fwd",
            Some(IconName::ArrowRight),
            false,
            window,
            cx,
        );
        nav.child(div().h(px(20.)))
            .child(h_flex().gap_1().child(back).child(fwd))
            .into_any_element()
    }

    fn content(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let page = match self.route() {
            Route::Library => self.library(window, cx),
            Route::Settings => self.settings(window, cx),
            Route::Console => div()
                .size_full()
                .p_2()
                .child(self.console.clone())
                .into_any_element(),
        };
        div()
            .flex_1()
            .min_w_0()
            .h_full()
            .child(page)
            .into_any_element()
    }

    fn inspector(&mut self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.selected.and_then(|i| self.summaries.get(i));
        let title = selected.map_or("Nothing selected".to_owned(), |s| s.title.clone());
        let app_id = selected.map_or(String::new(), |s| format!("App ID {}", s.app_id));
        let a0 = self.button(
            Target::InspectorAction(0),
            "Back to grid",
            None,
            false,
            window,
            cx,
        );
        let a1 = self.button(
            Target::InspectorAction(1),
            "Command palette",
            Some(IconName::Search),
            false,
            window,
            cx,
        );
        v_flex()
            .w(px(INSPECTOR_W))
            .flex_shrink_0()
            .p_2()
            .gap_2()
            .bg(rgb(PANEL))
            .child(div().text_base().child("Inspector"))
            .child(title)
            .child(div().text_xs().child(app_id))
            .child(a0)
            .child(a1)
            .child(div().text_xs().child(format!(
                "Covers cached: {} ({:.1} MiB, {} evicted)",
                self.covers.len(),
                self.covers.bytes() as f64 / 1_048_576.0,
                self.covers.evictions
            )))
            .child(div().text_xs().child(self.renderer.clone()))
            .child(
                div()
                    .text_xs()
                    .child(format!("focus zone: {}", self.zone_label())),
            )
            .into_any_element()
    }

    fn status_bar(&self, cx: &Context<Self>) -> AnyElement {
        let lines = self.console.read(cx).console.ring.len();
        div()
            .px_2()
            .py_1()
            .text_xs()
            .bg(rgb(PANEL))
            .child(format!("{} · console {lines} lines", self.status))
            .into_any_element()
    }
}

/// The modal: a GPUI Kit `Dialog` (Root focus trap for Tab, Escape closes). B on the gamepad
/// dispatches `Back`, which closes it through the shell.
pub fn modal_body(
    dialog: Dialog,
    shell: &Entity<Shell>,
    b0: &FocusHandle,
    b1: &FocusHandle,
    registry: &Registry,
    window: &Window,
) -> Dialog {
    let pick = {
        let shell = shell.clone();
        Button::new("modal-browse")
            .label("Browse…")
            .icon(IconName::Folder)
            .tab_stop(false)
            .on_click(move |_, window, cx| shell.update(cx, |s, cx| s.pick_file(window, cx)))
    };
    let close = {
        let shell = shell.clone();
        Button::new("modal-close")
            .label("Close")
            .icon(IconName::Close)
            .tab_stop(false)
            .on_click(move |_, window, cx| shell.update(cx, |s, cx| s.close_overlay(window, cx)))
    };
    dialog
        .title("Pick game executable")
        .w(px(420.))
        .child(
            div()
                .text_xs()
                .child("Opens the xdg-desktop-portal file chooser (no GTK)."),
        )
        .child(super::nav::nav_actions(
            h_flex()
                .gap_2()
                .child(wrap(Target::Overlay(0), Some(b0), registry, window, pick))
                .child(wrap(Target::Overlay(1), Some(b1), registry, window, close)),
            shell,
        ))
}

/// The command palette: a Kit `Dialog` holding a Kit `Input`; Enter navigates.
pub fn palette_body(dialog: Dialog, palette: &Entity<InputState>, shell: &Entity<Shell>) -> Dialog {
    dialog
        .title("Go to…")
        .w(px(420.))
        .child(super::nav::nav_actions(
            div()
                .id("palette-body")
                .child(Input::new(palette).id("palette")),
            shell,
        ))
}
