//! Lucide icon subset (ISC, `assets/icons/LICENSE`), embedded and rendered with `svg`.
//! `currentColor` is replaced at load so the icons render in the theme's text colour.

use std::sync::OnceLock;

use iced::widget::svg;

#[derive(Debug, Clone, Copy)]
pub enum Icon {
    Library,
    Settings,
    Terminal,
    Star,
    StarOff,
    Search,
    Close,
    Folder,
}

const COUNT: usize = 8;

fn source(icon: Icon) -> &'static str {
    match icon {
        Icon::Library => include_str!("../assets/icons/library-big.svg"),
        Icon::Settings => include_str!("../assets/icons/settings.svg"),
        Icon::Terminal => include_str!("../assets/icons/terminal.svg"),
        Icon::Star => include_str!("../assets/icons/star.svg"),
        Icon::StarOff => include_str!("../assets/icons/star-off.svg"),
        Icon::Search => include_str!("../assets/icons/search.svg"),
        Icon::Close => include_str!("../assets/icons/x.svg"),
        Icon::Folder => include_str!("../assets/icons/folder.svg"),
    }
}

pub fn handle(icon: Icon) -> svg::Handle {
    static HANDLES: OnceLock<Vec<svg::Handle>> = OnceLock::new();
    let all = HANDLES.get_or_init(|| {
        [
            Icon::Library,
            Icon::Settings,
            Icon::Terminal,
            Icon::Star,
            Icon::StarOff,
            Icon::Search,
            Icon::Close,
            Icon::Folder,
        ]
        .into_iter()
        .map(|i| {
            let s = source(i).replace("currentColor", "#e6ebf2");
            svg::Handle::from_memory(s.into_bytes())
        })
        .collect()
    });
    debug_assert_eq!(all.len(), COUNT);
    all[icon as usize].clone()
}

pub fn icon<'a>(icon: Icon, size: f32) -> iced::widget::Svg<'a> {
    iced::widget::svg(handle(icon)).width(size).height(size)
}
