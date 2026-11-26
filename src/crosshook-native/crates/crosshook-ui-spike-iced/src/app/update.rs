//! `update`: library, console, form, overlays. Focus movement lives in `navigation`.

use iced::Task;

use super::{App, Message, Overlay, Route};
use crate::console::replay::{ReplayEvent, ReplayPlan};
use crate::focus::Target;
use crate::form::Draft;
use crate::library::{covers, data, grid};

impl App {
    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Loaded(Ok(summaries)) => {
                self.status = format!("{} profiles", summaries.len());
                self.summaries = summaries;
                self.refilter();
                self.flow.loaded();
                let mut tasks = vec![self.pump_covers()];
                // M0-09 probe: put focus in the search field for scripted IME input.
                if std::env::var_os("SPIKE_FOCUS_SEARCH").is_some() {
                    tasks.push(self.focus_to(Target::Search));
                }
                if let Some(first) = self.summaries.first() {
                    let name = first.name.clone();
                    self.form_profile = Some(name.clone());
                    let paths = self.paths.clone();
                    tasks.push(blocking(
                        move || data::load_gamescope(&paths, &name),
                        Message::GamescopeLoaded,
                    ));
                }
                Task::batch(tasks)
            }
            Message::Loaded(Err(error)) => {
                self.status = format!("Library failed to load: {error}");
                self.flow.loaded();
                Task::none()
            }
            Message::CoverDecoded(key, result) => {
                self.decoding.remove(&key);
                match result {
                    Ok(decoded) => self.covers.insert(key, decoded),
                    Err(error) => eprintln!("spike: {error}"),
                }
                self.pump_covers()
            }
            Message::LibraryScrolled(viewport) => {
                self.scroll_y = viewport.absolute_offset().y;
                self.view_h = viewport.bounds.height;
                self.view_w = viewport.bounds.width;
                self.pump_covers()
            }
            Message::Search(query) => {
                // M0-09 probe: IME commit evidence.
                eprintln!("SPIKE_SEARCH_TEXT {query:?}");
                self.query = query;
                self.refilter();
                self.focus.set(Target::Search);
                let mark = self.bench_rendered("search");
                Task::batch([mark, self.pump_covers()])
            }
            Message::ToggleFavorite(display) => {
                if let Some(&idx) = self.filtered.get(display)
                    && !self.favorites.remove(&idx)
                {
                    self.favorites.insert(idx);
                }
                self.bench_rendered("toggle_favorite")
            }
            Message::Select(display) => {
                self.selected = self.filtered.get(display).copied();
                Task::none()
            }
            Message::Navigate(route) => {
                self.history.push(route);
                self.bench_rendered("route_switch")
            }
            Message::Back => {
                self.history.back();
                Task::none()
            }
            Message::Forward => {
                self.history.forward();
                Task::none()
            }
            Message::Resized(size) => {
                self.window = size;
                Task::none()
            }
            Message::WindowFocus(true) => {
                self.seen_focus = true;
                Task::none()
            }
            Message::WindowFocus(false) => {
                self.flow.focus_lost(self.seen_focus);
                Task::none()
            }
            Message::Frame => self.on_frame(),
            Message::Replay(ReplayEvent::Batch(lines)) => {
                self.console.append(lines);
                Task::none()
            }
            Message::Replay(ReplayEvent::Done { sent }) => {
                self.console.done = true;
                self.status = format!("Replay done: {sent} lines");
                Task::none()
            }
            Message::Replay(ReplayEvent::Failed(error)) => {
                self.status = format!("Replay failed: {error}");
                Task::none()
            }
            Message::ConsoleScrolled(scroll) => {
                self.console.scroll = scroll;
                Task::none()
            }
            Message::GamescopeLoaded(Ok(cfg)) => {
                self.draft = Draft::from_config(cfg);
                Task::none()
            }
            Message::GamescopeLoaded(Err(error)) | Message::Saved(Err(error)) => {
                self.status = format!("Profile error: {error}");
                Task::none()
            }
            Message::Number(i, value) => {
                if let Some(slot) = self.draft.numbers.get_mut(i) {
                    *slot = value;
                }
                self.focus.set(Target::Field(i as u8));
                Task::none()
            }
            Message::Enabled(on) => {
                self.draft.base.enabled = on;
                Task::none()
            }
            Message::Filter(filter) => {
                self.draft.base.upscale_filter = Some(filter.0);
                Task::none()
            }
            Message::Flag(n, on) => {
                let b = &mut self.draft.base;
                match n {
                    8 => b.fullscreen = on,
                    9 => b.borderless = on,
                    10 => b.grab_cursor = on,
                    _ => b.hdr_enabled = on,
                }
                Task::none()
            }
            Message::Optimization(on) => {
                self.draft.optimization = on;
                self.bench_rendered("toggle_optimization")
            }
            Message::Save => self.save(),
            Message::Saved(Ok(())) => {
                self.status = "Profile saved".into();
                Task::none()
            }
            Message::OpenModal => self.open_overlay(
                Overlay::Modal {
                    opener: self.focus.focused.clone(),
                },
                Target::Overlay(0),
            ),
            Message::OpenPalette => {
                self.palette.clear();
                self.open_overlay(
                    Overlay::Palette {
                        opener: self.focus.focused.clone(),
                    },
                    Target::Palette,
                )
            }
            Message::Palette(value) => {
                self.palette = value;
                Task::none()
            }
            Message::PaletteSubmit => {
                let query = self.palette.trim().to_lowercase();
                let route = Route::ALL
                    .into_iter()
                    .find(|r| !query.is_empty() && r.label().to_lowercase().starts_with(&query));
                let close = self.close_overlay();
                match route {
                    Some(route) => Task::batch([close, self.update(Message::Navigate(route))]),
                    None => close,
                }
            }
            Message::CloseModal => self.close_overlay(),
            Message::PickFile => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Pick a game executable")
                        .pick_file()
                        .await
                        .map(|h| h.path().display().to_string())
                },
                Message::Picked,
            ),
            Message::Picked(path) => {
                self.status =
                    path.map_or_else(|| "No file picked".into(), |p| format!("Picked {p}"));
                if std::env::var_os("SPIKE_SMOKE_PICK").is_some() {
                    eprintln!("SPIKE_PICK_RESULT {}", self.status);
                }
                Task::none()
            }
            Message::Renderer(info) => {
                if self.bench.enabled || std::env::var_os("SPIKE_LOG_RENDERER").is_some() {
                    eprintln!("SPIKE_RENDERER {info}");
                }
                self.renderer = info;
                Task::none()
            }
            Message::Nav(nav) => self.on_nav(nav),
            Message::Tab { forward } => self.collect(super::Pending::Tab(forward)),
            Message::Escape => self.on_escape(),
            Message::Collected(items, pending) => self.on_collected(&items, pending),
            Message::FocusTarget(target) => self.focus_to(target),
        }
    }

    pub fn refilter(&mut self) {
        let q = self.query.to_lowercase();
        self.filtered = (0..self.summaries.len())
            .filter(|&i| q.is_empty() || self.summaries[i].title.to_lowercase().contains(&q))
            .collect();
    }

    /// Queues decodes for visible (+ overscan) cards and keeps at most `MAX_DECODES` in flight.
    pub fn pump_covers(&mut self) -> Task<Message> {
        let g = grid::Grid::new(self.view_w, self.filtered.len());
        let (first, last) = g.visible_items(self.scroll_y, self.view_h);
        for display in first..last {
            let Some(&idx) = self.filtered.get(display) else {
                break;
            };
            let key = self.summaries[idx].app_id.clone();
            if self.covers.get(&key).is_none()
                && !self.decoding.contains(&key)
                && !self.decode_queue.contains(&key)
            {
                self.decode_queue.push_back(key);
            }
        }
        let mut tasks = Vec::new();
        while self.decoding.len() < super::MAX_DECODES {
            let Some(key) = self.decode_queue.pop_front() else {
                break;
            };
            self.decoding.insert(key.clone());
            let path = covers::cover_path(&self.paths.data_home, &key);
            tasks.push(blocking(
                move || covers::decode(&path),
                move |r| Message::CoverDecoded(key.clone(), r),
            ));
        }
        Task::batch(tasks)
    }

    fn save(&mut self) -> Task<Message> {
        let (Some(name), Some(cfg)) = (self.form_profile.clone(), self.draft.to_config()) else {
            self.status = "Fix the highlighted fields first".into();
            return Task::none();
        };
        let paths = self.paths.clone();
        blocking(
            move || data::save_gamescope(&paths, &name, cfg),
            Message::Saved,
        )
    }

    pub fn open_overlay(&mut self, overlay: Overlay, first: Target) -> Task<Message> {
        self.overlay = Some(overlay);
        self.focus.overlay_open = true;
        self.focus_to(first)
    }

    pub fn close_overlay(&mut self) -> Task<Message> {
        let opener = match self.overlay.take() {
            Some(Overlay::Modal { opener } | Overlay::Palette { opener }) => opener,
            None => return Task::none(),
        };
        self.focus.close_overlay(opener.clone());
        opener.map_or_else(Task::none, |t| self.focus_to(t))
    }

    fn on_frame(&mut self) -> Task<Message> {
        if self.flow.frame() {
            return self.after_ready();
        }
        self.g8_poll()
    }

    pub fn start_replay(&mut self, gated: bool) {
        if let Some(path) = self.bench.log.clone() {
            self.replay = Some(ReplayPlan {
                path,
                rate: self.bench.rate,
                gated,
                markers: gated,
            });
        }
    }
}

/// Runs blocking work on tokio's blocking pool, never on the UI thread.
pub fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
    map: impl Fn(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(f)
                .await
                .map_err(|e| e.to_string())?
        },
        map,
    )
}
