//! THROWAWAY iced decision spike (M0-04, YAN-773). Never merged to `main`.
//!
//! Library grid (500 culled cards, async covers, byte-capped LRU), log console (paced replay,
//! 10k ring, culled view), Gamescope form on core types, modal + palette with focus trap,
//! focus zones driven by keyboard and gilrs, renderer override, portal file dialog, Lucide
//! icons, back/forward history, and the bench READY/BENCH_* protocol.

mod app;
mod bench;
mod console;
mod focus;
mod form;
mod gamepad;
mod icons;
mod library;
mod nav_history;
mod renderer;
mod settings_view;

use iced::{Size, window};

fn main() -> iced::Result {
    bench::init_clock();
    let args: Vec<String> = std::env::args().collect();
    if let Err(error) = renderer::apply(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    let window = window::Settings {
        size: Size::new(1280.0, 900.0),
        platform_specific: window::settings::PlatformSpecific {
            application_id: "dev.crosshook.CrossHook".into(),
            ..Default::default()
        },
        ..window::Settings::default()
    };
    iced::application(app::App::boot, app::App::update, app::App::view)
        .title(app::App::title)
        .subscription(app::App::subscription)
        .theme(iced::Theme::Dark)
        .window(window)
        .run()
}
