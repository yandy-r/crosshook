//! Gamescope settings: GPUI Kit `Form` + `field()` rows with `NumberInput`, `Select` and
//! `Switch` on the core `GamescopeConfig` draft (same field numbering as the iced spike).

use gpui::{
    AnyElement, Context, InteractiveElement as _, IntoElement as _, ParentElement,
    StatefulInteractiveElement as _, Styled, Window, div, px, rgb,
};
use gpui_component::button::{Button, ButtonVariants as _};
use gpui_component::form::{field, v_form};
use gpui_component::input::NumberInput;
use gpui_component::select::Select;
use gpui_component::switch::Switch;
use gpui_component::{IconName, v_flex};

use super::Shell;
use crate::focus::{Target, wrap};
use crate::form::NUMERIC;

impl Shell {
    pub fn settings(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let errors = self.draft.errors();
        let mut form = v_form().label_width(px(180.));
        form = form.child(
            field()
                .label("Enable gamescope")
                .child(self.switch(6, "", window, cx)),
        );
        for (i, label) in NUMERIC.iter().enumerate() {
            let input = wrap(
                Target::Field(i as u8),
                None,
                &self.registry,
                window,
                NumberInput::new(&self.numbers[i]).w(px(220.)),
            );
            let mut f = field().label(*label).child(input);
            if let Some(err) = &errors[i] {
                f = f.description(err.clone());
            }
            form = form.child(f);
        }
        let select = wrap(
            Target::Field(7),
            None,
            &self.registry,
            window,
            Select::new(&self.filter).w(px(220.)),
        );
        form = form.child(field().label("Upscale filter").child(select));
        for (n, label) in [
            (8u8, "Fullscreen"),
            (9, "Borderless"),
            (10, "Grab cursor"),
            (11, "HDR"),
        ] {
            form = form.child(field().label(label).child(self.switch(n, "", window, cx)));
        }
        form = form.child(field().label("Launch optimizations").child(self.switch(
            12,
            "Disable Steam overlay (optimization)",
            window,
            cx,
        )));
        let save_t = Target::Field(13);
        let save_h = self.handle(&save_t, cx);
        let save = Button::new("save")
            .label("Save")
            .primary()
            .tab_stop(false)
            .on_click(cx.listener(|s, _, w, cx| s.save(w, cx)));
        let pick_t = Target::Field(14);
        let pick_h = self.handle(&pick_t, cx);
        let pick = Button::new("pick")
            .icon(IconName::Folder)
            .label("Pick file…")
            .tab_stop(false)
            .on_click(cx.listener(|s, _, w, cx| {
                s.open_overlay(super::Overlay::Modal, w, cx);
            }));
        let mut page = v_flex()
            .gap_2()
            .p_3()
            .child(div().text_xl().child("Settings — Gamescope"))
            .child(div().text_xs().child(format!(
                "Profile: {}",
                self.form_profile.as_deref().unwrap_or("none")
            )))
            .child(form)
            .child(
                gpui_component::h_flex()
                    .gap_2p5()
                    .child(wrap(save_t, Some(&save_h), &self.registry, window, save))
                    .child(wrap(pick_t, Some(&pick_h), &self.registry, window, pick)),
            );
        // Extra height so the page scrolls (G4 `settings`), as in the iced spike.
        for i in 0..30 {
            page = page.child(
                div()
                    .text_sm()
                    .text_color(rgb(0x9aa4b2))
                    .child(format!("Advanced option placeholder {i}")),
            );
        }
        page = page.child(div().h(px(200.)));
        div()
            .id("settings-scroll")
            .size_full()
            .overflow_y_scroll()
            .track_scroll(&self.settings_scroll)
            .child(page)
            .into_any_element()
    }

    /// A Kit `Switch` driven by the draft. The wrapper owns the focus handle so the D-pad and
    /// zones can address it; A/Enter on the wrapper toggles it through `activate`.
    fn switch(
        &mut self,
        n: u8,
        label: &str,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let target = Target::Field(n);
        let handle = self.handle(&target, cx);
        let sw = Switch::new(("flag", n as usize))
            .checked(self.flag(n))
            .label(label.to_owned())
            .tab_stop(false)
            .on_change(cx.listener(move |s, on: &bool, _, cx| s.set_flag(n, *on, cx)));
        wrap(target, Some(&handle), &self.registry, window, sw)
    }
}
