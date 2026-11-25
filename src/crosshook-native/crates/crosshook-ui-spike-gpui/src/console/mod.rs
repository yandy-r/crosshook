//! Console: 10k-line ring buffer (from the iced spike), paced replay and the culled view.
//!
//! The view is GPUI's own `uniform_list`: it lays out and shapes only the visible rows, so a
//! hand-written culling widget (iced spike `log_view.rs`, 190 lines) is not needed here.

pub mod replay;
pub mod ring;
pub mod view;

pub use ring::Ring;

/// Console state owned by the shell.
#[derive(Debug, Default)]
pub struct Console {
    pub ring: Ring,
    pub done: bool,
    /// Follow the tail (scroll to bottom on append) until the user scrolls up.
    pub follow: bool,
}

impl Console {
    pub fn new() -> Self {
        Self {
            follow: true,
            ..Self::default()
        }
    }

    pub fn append(&mut self, batch: Vec<String>) {
        self.ring.extend(batch);
    }
}
