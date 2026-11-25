//! Console: ring buffer + culled log view + paced replay subscription.

pub mod log_view;
pub mod replay;
pub mod ring;

pub use log_view::{LogScroll, log_view};
pub use ring::Ring;

/// Console state owned by the app.
#[derive(Debug, Default)]
pub struct Console {
    pub ring: Ring,
    pub scroll: LogScroll,
    pub done: bool,
}

impl Console {
    /// Appends one coalesced batch. While the user is scrolled up the view keeps its place:
    /// `from_bottom` grows by the appended height (until the ring starts dropping lines).
    pub fn append(&mut self, batch: Vec<String>) {
        let n = batch.len() as f32;
        self.ring.extend(batch);
        if !self.scroll.following() {
            self.scroll.from_bottom += n * log_view::LINE_HEIGHT;
        }
    }
}
