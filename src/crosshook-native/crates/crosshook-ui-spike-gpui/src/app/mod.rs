//! The shell entity: state, boot (core load on tokio via `core_bridge`, cover decode on gpui's
//! background executor, replay and gamepad channels drained by foreground tasks).
//! `view`/`library`/`settings` render it, `nav` routes focus, `bench_flow` drives the probe.

mod bench_flow;
mod library;
mod nav;
mod settings;
#[cfg(test)]
mod tests;
mod view;

use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use futures::StreamExt as _;
use futures::channel::mpsc;
use gpui::{
    App, AppContext as _, Context, Entity, FocusHandle, Pixels, SharedString, Size, Subscription,
    Task, WeakEntity, Window, px,
};
use gpui_base::History;
use gpui_component::IndexPath;
use gpui_component::VirtualListScrollHandle;
use gpui_component::input::{InputEvent, InputState};
use gpui_component::searchable_list::SearchableVec;
use gpui_component::select::{SelectEvent, SelectState};

use crate::actions::Nav;
use crate::bench::BenchConfig;
use crate::console::replay::{self, ReplayEvent, ReplayPlan};
use crate::console::view::ConsoleView;
use crate::core_bridge::{self, Paths, Summary};
use crate::focus::{FocusState, Registry, Target};
use crate::form::{Draft, FILTERS, NUMERIC};
use crate::library::covers::{self, CoverCache};
use crate::library::grid::{self, Grid};

pub use bench_flow::BenchFlow;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    Modal,
    Palette,
}

pub type FilterSelect = SelectState<SearchableVec<SharedString>>;

pub struct Shell {
    pub bench: BenchConfig,
    pub flow: BenchFlow,
    pub paths: Option<Paths>,
    pub summaries: Vec<Summary>,
    pub filtered: Vec<usize>,
    pub favorites: HashSet<usize>,
    pub selected: Option<usize>,
    pub covers: CoverCache,
    pub decoding: HashSet<String>,
    pub decode_queue: VecDeque<String>,
    pub grid_scroll: VirtualListScrollHandle,
    pub settings_scroll: gpui::ScrollHandle,
    /// Columns of the last rendered grid (the D-pad grid model needs it between frames).
    pub cols: usize,
    pub row_sizes: Rc<Vec<Size<Pixels>>>,
    pub history: History<Route>,
    pub focus: FocusState,
    pub registry: Registry,
    /// Focus handles of spike-owned targets (cards, buttons, switches). Kit inputs own theirs.
    pub handles: HashMap<Target, FocusHandle>,
    /// Focus handle of the shell root (fallback focus so actions reach the shell).
    pub root_focus: FocusHandle,
    pub overlay: Option<Overlay>,
    /// Target focused before the overlay opened; focus returns there on close.
    pub opener: Option<Target>,
    pub search: Entity<InputState>,
    /// Lower-cased search text, mirrored from the input's change events.
    pub query: String,
    pub palette: Entity<InputState>,
    pub numbers: Vec<Entity<InputState>>,
    pub filter: Entity<FilterSelect>,
    pub draft: Draft,
    pub form_profile: Option<String>,
    pub console: Entity<ConsoleView>,
    pub status: SharedString,
    pub renderer: SharedString,
    pub seen_focus: bool,
    /// Next G5 trial to arm after the change frame was built.
    pub rearm: Option<u64>,
    tasks: Vec<Task<()>>,
    _subs: Vec<Subscription>,
}

impl Shell {
    /// `boot` loads profiles through core and starts gilrs; tests pass `false`.
    pub fn new(boot: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let bench = BenchConfig::from_env();
        let flow = BenchFlow::new(&bench);
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search games"));
        let palette =
            cx.new(|cx| InputState::new(window, cx).placeholder("library, settings, console"));
        let numbers: Vec<_> = NUMERIC
            .iter()
            .map(|label| cx.new(|cx| InputState::new(window, cx).placeholder(*label)))
            .collect();
        let names: Vec<SharedString> = FILTERS.iter().map(|f| f.to_string().into()).collect();
        let filter = cx.new(|cx| {
            SelectState::new(
                SearchableVec::new(names),
                Some(IndexPath::new(0)),
                window,
                cx,
            )
        });
        let mut subs = vec![
            cx.subscribe_in(&search, window, Self::on_search_event),
            cx.subscribe_in(&palette, window, Self::on_palette_event),
            cx.subscribe_in(&filter, window, Self::on_filter_event),
            cx.observe_window_activation(window, |this, window, _| {
                if window.is_window_active() {
                    this.seen_focus = true;
                } else {
                    this.flow.focus_lost(this.seen_focus);
                }
            }),
        ];
        for (i, input) in numbers.iter().enumerate() {
            subs.push(
                cx.subscribe_in(input, window, move |this, state, ev, _, cx| {
                    if matches!(ev, InputEvent::Change) {
                        this.draft.numbers[i] = state.read(cx).value().to_string();
                        cx.notify();
                    }
                }),
            );
        }
        let mut history = History::new().max_entries(50);
        history.push(Route::Library);
        let mut shell = Self {
            bench,
            flow,
            paths: None,
            summaries: Vec::new(),
            filtered: Vec::new(),
            favorites: HashSet::new(),
            selected: None,
            covers: CoverCache::new(covers::BUDGET_BYTES),
            decoding: HashSet::new(),
            decode_queue: VecDeque::new(),
            grid_scroll: VirtualListScrollHandle::new(),
            settings_scroll: gpui::ScrollHandle::new(),
            cols: 1,
            row_sizes: Rc::new(Vec::new()),
            history,
            focus: FocusState::default(),
            registry: Registry::default(),
            handles: HashMap::new(),
            root_focus: cx.focus_handle(),
            overlay: None,
            opener: None,
            search,
            query: String::new(),
            palette,
            numbers,
            filter,
            draft: Draft::default(),
            form_profile: None,
            console: cx.new(|_| ConsoleView::new()),
            status: "Loading library…".into(),
            renderer: SharedString::default(),
            seen_focus: false,
            rearm: None,
            tasks: Vec::new(),
            _subs: subs,
        };
        if boot {
            shell.boot(window, cx);
        }
        shell
    }

    fn boot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Core work runs on the tokio blocking pool via core_bridge, never on this thread.
        let load = core_bridge::blocking(cx, core_bridge::load_summaries);
        self.tasks.push(cx.spawn_in(window, async move |this, cx| {
            let result = load.await;
            let _ = this.update_in(cx, |this, window, cx| match result {
                Ok((paths, summaries)) => this.set_library(Some(paths), summaries, window, cx),
                Err(e) => {
                    this.status = format!("Library failed to load: {e}").into();
                    this.flow.loaded();
                    cx.notify();
                }
            });
        }));
        // Gamepad: gilrs thread -> channel -> the same gpui actions the keyboard dispatches.
        let (tx, mut rx) = mpsc::unbounded::<Nav>();
        crate::gamepad::spawn(tx);
        self.tasks.push(cx.spawn_in(window, async move |_, cx| {
            while let Some(nav) = rx.next().await {
                let sent = cx.update(|window, cx| {
                    window.dispatch_action(crate::actions::boxed(nav), cx);
                });
                if sent.is_err() {
                    break;
                }
            }
        }));
        let info = crate::renderer::describe(window.gpu_specs());
        if crate::renderer::renderer_log_on() {
            eprintln!("SPIKE_RENDERER {info}");
        }
        self.renderer = info.into();
        // Spike evidence hook: open the portal file chooser at startup (Flatpak smoke test).
        if std::env::var_os("SPIKE_SMOKE_PICK").is_some() {
            self.pick_file(window, cx);
        }
    }

    /// Installs a loaded library (tests call it with synthetic summaries and no paths).
    pub fn set_library(
        &mut self,
        paths: Option<Paths>,
        summaries: Vec<Summary>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = format!("{} profiles", summaries.len()).into();
        self.summaries = summaries;
        self.paths = paths.clone();
        self.refilter();
        if let (Some(paths), Some(first)) = (paths, self.summaries.first()) {
            let name = first.name.clone();
            self.form_profile = Some(name.clone());
            let job = core_bridge::blocking(cx, move || core_bridge::load_gamescope(&paths, &name));
            self.tasks.push(cx.spawn_in(window, async move |this, cx| {
                let result = job.await;
                let _ = this.update_in(cx, |this, window, cx| {
                    match result {
                        Ok(cfg) => this.set_draft(Draft::from_config(cfg), window, cx),
                        Err(e) => this.status = format!("Profile error: {e}").into(),
                    }
                    cx.notify();
                });
            }));
        }
        self.flow.loaded();
        cx.notify();
    }

    fn set_draft(&mut self, draft: Draft, window: &mut Window, cx: &mut Context<Self>) {
        for (i, input) in self.numbers.iter().enumerate() {
            let value = draft.numbers[i].clone();
            input.update(cx, |s, cx| s.set_value(value, window, cx));
        }
        let idx = draft
            .base
            .upscale_filter
            .as_ref()
            .and_then(|cur| FILTERS.iter().position(|f| f.0 == *cur));
        if let Some(idx) = idx {
            self.filter.update(cx, |s, cx| {
                s.set_selected_index(Some(IndexPath::new(idx)), window, cx);
            });
        }
        self.draft = draft;
    }

    pub fn route(&self) -> Route {
        self.history.current().copied().unwrap_or(Route::Library)
    }

    pub fn wide(&self, window: &Window) -> bool {
        window.viewport_size().width >= px(WIDE_LAYOUT)
    }

    pub fn refilter(&mut self) {
        let q = self.query.clone();
        self.filtered = (0..self.summaries.len())
            .filter(|&i| q.is_empty() || self.summaries[i].title.to_lowercase().contains(&q))
            .collect();
    }

    pub fn grid_model(&self) -> Grid {
        Grid {
            cols: self.cols.max(1),
            items: self.filtered.len(),
        }
    }

    /// Handle for a spike-owned target, created on first use (stable across frames).
    pub fn handle(&mut self, target: &Target, cx: &mut App) -> FocusHandle {
        self.handles
            .entry(target.clone())
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    fn on_search_event(
        &mut self,
        state: &Entity<InputState>,
        ev: &InputEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(ev, InputEvent::Change) {
            self.query = state.read(cx).value().to_lowercase();
            self.refilter();
            self.grid_scroll
                .scroll_to_item(0, gpui::ScrollStrategy::Top);
            self.bench_rendered("search");
            cx.notify();
        }
    }

    fn on_palette_event(
        &mut self,
        state: &Entity<InputState>,
        ev: &InputEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Inside Kit's dialog Enter is the dialog's `Confirm` (handled by `palette_submit`
        // through `on_ok`); this path only fires if the input is used outside a dialog.
        if let InputEvent::PressEnter { .. } = ev {
            let q = state.read(cx).value().to_string();
            self.palette_submit(&q, window, cx);
        }
    }

    /// Palette submit: close the overlay, then go to the first route whose label starts with `q`.
    pub fn palette_submit(&mut self, q: &str, window: &mut Window, cx: &mut Context<Self>) {
        let q = q.trim().to_lowercase();
        let route = Route::ALL
            .into_iter()
            .find(|r| !q.is_empty() && r.label().to_lowercase().starts_with(&q));
        self.overlay_closed(window, cx);
        if let Some(route) = route {
            self.navigate(route, cx);
        }
    }

    fn on_filter_event(
        &mut self,
        _: &Entity<FilterSelect>,
        ev: &SelectEvent<SearchableVec<SharedString>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let SelectEvent::Confirm(Some(name)) = ev else {
            return;
        };
        if let Some(f) = FILTERS.iter().find(|f| f.to_string() == name.as_ref()) {
            self.draft.base.upscale_filter = Some(f.0.clone());
            cx.notify();
        }
    }

    /// Queues decodes for the rows the virtual list renders (+ overscan). At most
    /// `MAX_DECODES` run at once, on gpui's background executor (pure `image` work).
    pub fn want_rows(&mut self, rows: std::ops::Range<usize>, cx: &mut Context<Self>) {
        let cols = self.cols.max(1);
        let first = rows.start.saturating_sub(grid::OVERSCAN_ROWS) * cols;
        let last = ((rows.end + grid::OVERSCAN_ROWS) * cols).min(self.filtered.len());
        for display in first..last {
            let key = self.summaries[self.filtered[display]].app_id.clone();
            if !self.covers.contains(&key)
                && !self.decoding.contains(&key)
                && !self.decode_queue.contains(&key)
            {
                self.decode_queue.push_back(key);
            }
        }
        self.pump_covers(cx);
    }

    fn pump_covers(&mut self, cx: &mut Context<Self>) {
        let Some(paths) = self.paths.clone() else {
            return;
        };
        while self.decoding.len() < MAX_DECODES {
            let Some(key) = self.decode_queue.pop_front() else {
                break;
            };
            self.decoding.insert(key.clone());
            let path = covers::cover_path(&paths.data_home, &key);
            let job = cx.background_spawn(async move { covers::decode(&path) });
            cx.spawn(async move |this: WeakEntity<Self>, cx| {
                let result = job.await;
                let _ = this.update(cx, |this, cx| {
                    this.decoding.remove(&key);
                    match result {
                        Ok(decoded) => {
                            for gone in this.covers.insert(key, decoded) {
                                cx.drop_image(gone, None);
                            }
                        }
                        Err(e) => eprintln!("spike: {e}"),
                    }
                    this.pump_covers(cx);
                    cx.notify();
                });
            })
            .detach();
        }
    }

    pub fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(name), Some(cfg), Some(paths)) = (
            self.form_profile.clone(),
            self.draft.to_config(),
            self.paths.clone(),
        ) else {
            self.status = "Fix the highlighted fields first".into();
            return;
        };
        let job =
            core_bridge::blocking(cx, move || core_bridge::save_gamescope(&paths, &name, cfg));
        self.tasks.push(cx.spawn_in(window, async move |this, cx| {
            let result = job.await;
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(()) => "Profile saved".into(),
                    Err(e) => format!("Profile error: {e}").into(),
                };
                cx.notify();
            });
        }));
    }

    /// xdg-desktop-portal file chooser through gpui's own `prompt_for_paths`
    /// (gpui-pre-linux uses `ashpd::desktop::file_chooser`; no gtk3 in the tree).
    pub fn pick_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let rx = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Pick a game executable".into()),
        });
        self.tasks.push(cx.spawn_in(window, async move |this, cx| {
            let status: SharedString = match rx.await {
                Ok(Ok(Some(paths))) => paths.first().map_or("No file picked".into(), |p| {
                    format!("Picked {}", p.display()).into()
                }),
                Ok(Ok(None)) => "No file picked".into(),
                Ok(Err(e)) => format!("Portal error: {e}").into(),
                Err(_) => "Portal request dropped".into(),
            };
            if std::env::var_os("SPIKE_SMOKE_PICK").is_some() {
                eprintln!("SPIKE_PICK_RESULT {status}");
            }
            let _ = this.update(cx, |this, cx| {
                this.status = status;
                cx.notify();
            });
        }));
    }

    /// Paced replay: a reader thread feeds a channel; a foreground task appends batches.
    pub fn start_replay(&mut self, gated: bool, cx: &mut Context<Self>) {
        let Some(path) = self.bench.log.clone() else {
            return;
        };
        let (tx, mut rx) = mpsc::unbounded();
        replay::spawn(
            ReplayPlan {
                path,
                rate: self.bench.rate,
                gated,
                markers: gated,
            },
            tx,
        );
        let console = self.console.clone();
        self.tasks
            .push(cx.spawn(async move |this: WeakEntity<Self>, cx| {
                while let Some(event) = rx.next().await {
                    let alive = match event {
                        ReplayEvent::Batch(lines) => {
                            console.update(cx, |c, cx| c.append(lines, cx));
                            true
                        }
                        ReplayEvent::Done { sent } => this
                            .update(cx, |this, cx| {
                                this.console.update(cx, |c, _| c.console.done = true);
                                this.status = format!("Replay done: {sent} lines").into();
                                cx.notify();
                            })
                            .is_ok(),
                        ReplayEvent::Failed(e) => this
                            .update(cx, |this, cx| {
                                this.status = format!("Replay failed: {e}").into();
                                cx.notify();
                            })
                            .is_ok(),
                    };
                    if !alive {
                        break;
                    }
                }
            }));
    }

    pub fn navigate(&mut self, route: Route, cx: &mut Context<Self>) {
        if self.route() != route {
            self.history.push(route);
        }
        self.bench_rendered("route_switch");
        cx.notify();
    }

    pub fn toggle_favorite(&mut self, display: usize, cx: &mut Context<Self>) {
        if let Some(&idx) = self.filtered.get(display)
            && !self.favorites.remove(&idx)
        {
            self.favorites.insert(idx);
        }
        self.bench_rendered("toggle_favorite");
        cx.notify();
    }

    /// Form flags by field number: 6 enabled, 8..=11 fullscreen/borderless/grab/HDR,
    /// 12 launch optimization.
    pub fn set_flag(&mut self, n: u8, on: bool, cx: &mut Context<Self>) {
        let b = &mut self.draft.base;
        match n {
            6 => b.enabled = on,
            8 => b.fullscreen = on,
            9 => b.borderless = on,
            10 => b.grab_cursor = on,
            11 => b.hdr_enabled = on,
            _ => {
                self.draft.optimization = on;
                self.bench_rendered("toggle_optimization");
            }
        }
        cx.notify();
    }

    pub fn flag(&self, n: u8) -> bool {
        let b = &self.draft.base;
        match n {
            6 => b.enabled,
            8 => b.fullscreen,
            9 => b.borderless,
            10 => b.grab_cursor,
            11 => b.hdr_enabled,
            _ => self.draft.optimization,
        }
    }
}
