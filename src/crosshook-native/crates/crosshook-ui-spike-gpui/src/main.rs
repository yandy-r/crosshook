//! THROWAWAY gpui decision spike (M0-05, YAN-836). Never merged to `main`.
//!
//! Same slice as the iced spike (M0-04): library grid (500 cards, VirtualList, async covers,
//! byte-capped LRU), log console (paced replay, 10k ring, culled view), Gamescope form on core
//! types (GPUI Kit Form/NumberInput/Select/Switch), Kit Dialog modal and palette with focus
//! trap, focus zones driven by keyboard and gilrs, renderer override + adapter log, portal file
//! dialog, Lucide icons, back/forward history, and the bench READY/BENCH_* protocol.

mod actions;
mod app;
mod bench;
mod console;
mod core_bridge;
mod focus;
mod form;
mod gamepad;
mod library;
mod renderer;

use gpui::{AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    bench::init_clock();
    let args: Vec<String> = std::env::args().collect();
    if let Err(error) = renderer::apply(&args) {
        eprintln!("{error}");
        std::process::exit(2);
    }
    renderer::init_logger();
    core_bridge::mark_ui_thread();
    // crosshook-core enables zbus's `tokio` feature; feature unification turns it on for every
    // zbus user in the process (gpui-base reduce_motion, ashpd portals, accesskit_unix), which
    // then panic outside a tokio context ("there is no reactor running"). Entering the
    // app-owned runtime on the gpui main thread gives them one (finding F1 in the write-up).
    let runtime = core_bridge::build_runtime();
    let _tokio_context = runtime.enter();
    let handle = runtime.handle().clone();
    gpui_platform::application()
        .with_assets(gpui_kit_assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);
            actions::bind_keys(cx);
            // App-owned tokio runtime for crosshook-core, shared with gpui through gpui_tokio.
            core_bridge::init_from_handle(cx, handle);
            let options = WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(900.)),
                    cx,
                ))),
                app_id: Some("dev.crosshook.CrossHook".into()),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("CrossHook (gpui spike)".into()),
                    ..Default::default()
                }),
                ..WindowOptions::default()
            };
            let opened = gpui_kit::open_window(options, cx, |window, cx| {
                cx.new(|cx| app::Shell::new(true, window, cx))
            });
            if let Err(error) = opened {
                eprintln!("spike: could not open window: {error:#}");
                cx.quit();
            }
            cx.on_window_closed(|cx, _| cx.quit()).detach();
        });
}
