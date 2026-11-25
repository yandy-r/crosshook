//! App state, messages, boot and subscriptions. `update` and `view` live in sibling modules.

mod bench_flow;
mod keys;
mod navigation;
mod probe;
#[cfg(test)]
mod sim_tests;
mod update;
mod view;

use std::collections::{HashSet, VecDeque};

use iced::widget::Id;
use iced::{Size, Subscription, Task, window};

use crate::bench::BenchConfig;
use crate::console::Console;
use crate::console::replay::{self, ReplayEvent, ReplayPlan};
use crate::focus::{FocusState, Target};
use crate::form::Draft;
use crate::gamepad::{self, Nav};
use crate::library::covers::{CoverCache, Decoded};
use crate::library::data::{Paths, Summary};
use crate::nav_history::History;

pub use bench_flow::BenchFlow;

pub const LIBRARY_SCROLL: &str = "library-scroll";
pub const SETTINGS_SCROLL: &str = "settings-scroll";
pub const WIDE_LAYOUT: f32 = 1440.0;
pub const SIDEBAR_W: f32 = 200.0;
pub const INSPECTOR_W: f32 = 260.0;
pub const CONSOLE_H: f32 = 380.0;
pub const MAX_DECODES: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Route {
    Library,
    Settings,
    Console,
}

impl Route {
    pub const ALL: [Route; 3] = [Route::Library, Route::Settings, Route::Console];
    pub fn label(self) -> &'static str {
        match self {
            Route::Library => "Library",
            Route::Settings => "Settings",
            Route::Console => "Console",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Modal { opener: Option<Target> },
    Palette { opener: Option<Target> },
}

#[derive(Debug, Clone)]
pub enum Message {
    Loaded(Result<Vec<Summary>, String>),
    CoverDecoded(String, Result<Decoded, String>),
    LibraryScrolled(iced::widget::scrollable::Viewport),
    Search(String),
    ToggleFavorite(usize),
    Select(usize),
    Navigate(Route),
    Back,
    Forward,
    Nav(Nav),
    Tab { forward: bool },
    Escape,
    OpenPalette,
    Palette(String),
    PaletteSubmit,
    Collected(Vec<(Target, iced::Rectangle)>, Pending),
    FocusTarget(Target),
    Resized(Size),
    WindowFocus(bool),
    Frame,
    Replay(ReplayEvent),
    ConsoleScrolled(crate::console::LogScroll),
    // Form
    GamescopeLoaded(Result<crosshook_core::profile::GamescopeConfig, String>),
    Number(usize, String),
    Enabled(bool),
    Filter(crate::form::Filter),
    Flag(u8, bool),
    Optimization(bool),
    Save,
    Saved(Result<(), String>),
    OpenModal,
    PickFile,
    Picked(Option<String>),
    CloseModal,
    Renderer(String),
}

/// What to do with the next collected focus list.
#[derive(Debug, Clone, PartialEq)]
pub enum Pending {
    Nav(Nav),
    Tab(bool),
}

pub struct App {
    pub paths: Paths,
    pub bench: BenchConfig,
    pub flow: BenchFlow,
    pub summaries: Vec<Summary>,
    pub filtered: Vec<usize>,
    pub favorites: HashSet<usize>,
    pub query: String,
    pub selected: Option<usize>,
    pub covers: CoverCache,
    pub decoding: HashSet<String>,
    pub decode_queue: VecDeque<String>,
    pub scroll_y: f32,
    pub view_h: f32,
    pub view_w: f32,
    pub window: Size,
    pub history: History<Route>,
    pub focus: FocusState,
    pub overlay: Option<Overlay>,
    pub palette: String,
    pub console: Console,
    pub replay: Option<ReplayPlan>,
    pub draft: Draft,
    pub form_profile: Option<String>,
    pub status: String,
    pub renderer: String,
    pub seen_focus: bool,
}

impl App {
    pub fn boot() -> (Self, Task<Message>) {
        let paths = Paths::from_env();
        let bench = BenchConfig::from_env();
        let flow = BenchFlow::new(&bench);
        let load = {
            let paths = paths.clone();
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        crate::library::data::load_summaries(&paths)
                    })
                    .await
                    .map_err(|e| e.to_string())?
                },
                Message::Loaded,
            )
        };
        let info = iced::system::information().map(|info| {
            Message::Renderer(format!(
                "backend={} adapter={}",
                info.graphics_backend, info.graphics_adapter
            ))
        });
        let app = Self {
            paths,
            bench,
            flow,
            summaries: Vec::new(),
            filtered: Vec::new(),
            favorites: HashSet::new(),
            query: String::new(),
            selected: None,
            covers: CoverCache::new(crate::library::covers::BUDGET_BYTES),
            decoding: HashSet::new(),
            decode_queue: VecDeque::new(),
            scroll_y: 0.0,
            view_h: 900.0,
            view_w: 1280.0 - SIDEBAR_W - INSPECTOR_W,
            window: Size::new(1280.0, 900.0),
            history: History::new(Route::Library, 50),
            focus: FocusState::default(),
            overlay: None,
            palette: String::new(),
            console: Console::default(),
            replay: None,
            draft: Draft::default(),
            form_profile: None,
            status: "Loading library…".into(),
            renderer: String::new(),
            seen_focus: false,
        };
        // Spike evidence hook: open the portal file chooser at startup (Flatpak smoke test).
        let smoke = if std::env::var_os("SPIKE_SMOKE_PICK").is_some() {
            Task::done(Message::PickFile)
        } else {
            Task::none()
        };
        (app, Task::batch([load, info, smoke]))
    }

    pub fn route(&self) -> Route {
        *self.history.current()
    }

    pub fn wide(&self) -> bool {
        self.window.width >= WIDE_LAYOUT
    }

    pub fn title(&self) -> String {
        "CrossHook (iced spike)".into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subs = vec![
            keys::subscription(),
            gamepad::subscription().map(Message::Nav),
            window::resize_events().map(|(_, size)| Message::Resized(size)),
            window::events().filter_map(|(_, e)| match e {
                window::Event::Focused => Some(Message::WindowFocus(true)),
                window::Event::Unfocused => Some(Message::WindowFocus(false)),
                _ => None,
            }),
        ];
        if self.flow.wants_frames() {
            subs.push(window::frames().map(|_| Message::Frame));
        }
        if let Some(plan) = &self.replay {
            subs.push(replay::subscription(plan.clone()).map(Message::Replay));
        }
        Subscription::batch(subs)
    }

    pub fn focused_is(&self, target: &Target) -> bool {
        self.focus.focused.as_ref() == Some(target)
    }
}

pub fn scroll_id(route: Route) -> Id {
    Id::new(if route == Route::Settings {
        SETTINGS_SCROLL
    } else {
        LIBRARY_SCROLL
    })
}
