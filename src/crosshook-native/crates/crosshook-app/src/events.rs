//! Bounded in-process event fan-out.

use tokio::sync::broadcast::{self, error::RecvError};

use crate::error::AppError;

/// Capacity used by [`EventBus::default`].
pub const DEFAULT_EVENT_CAPACITY: usize = 256;

/// Largest capacity [`EventBus::new`] accepts.
pub const MAX_EVENT_CAPACITY: usize = 1 << 20;

/// Event delivered to subscribers.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppEvent {
    /// Subscriber fell behind and `missed` events were dropped; re-read state.
    Resync {
        /// Number of events the subscriber missed.
        missed: u64,
    },
}

/// Bounded broadcast bus. Slow subscribers lose the oldest events and are told
/// via [`AppEvent::Resync`]; publishers never block.
#[derive(Debug)]
pub struct EventBus {
    tx: broadcast::Sender<AppEvent>,
}

impl EventBus {
    /// Creates a bus retaining at most `capacity` events per lagging subscriber.
    ///
    /// # Errors
    /// Validation error when `capacity` is zero or above [`MAX_EVENT_CAPACITY`].
    pub fn new(capacity: usize) -> Result<Self, AppError> {
        if capacity == 0 || capacity > MAX_EVENT_CAPACITY {
            return Err(AppError::validation(
                "event_capacity",
                "event capacity must be between 1 and MAX_EVENT_CAPACITY",
            ));
        }
        Ok(Self {
            tx: broadcast::channel(capacity).0,
        })
    }

    /// Publishes `event`; returns how many subscribers it was queued for.
    /// Returns `0` (not an error) when nobody is subscribed.
    pub fn publish(&self, event: AppEvent) -> usize {
        // `send` only fails when there are no receivers; that is a normal state.
        self.tx.send(event).unwrap_or(0)
    }

    /// Subscribes from the next published event onward.
    pub fn subscribe(&self) -> EventReceiver {
        EventReceiver {
            rx: self.tx.subscribe(),
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            tx: broadcast::channel(DEFAULT_EVENT_CAPACITY).0,
        }
    }
}

/// Subscription to an [`EventBus`].
#[derive(Debug)]
pub struct EventReceiver {
    rx: broadcast::Receiver<AppEvent>,
}

impl EventReceiver {
    /// Awaits the next event. A lag is reported as [`AppEvent::Resync`] and the
    /// retained tail follows. `None` once the bus is dropped and drained.
    pub async fn recv(&mut self) -> Option<AppEvent> {
        match self.rx.recv().await {
            Ok(event) => Some(event),
            Err(RecvError::Lagged(missed)) => Some(AppEvent::Resync { missed }),
            Err(RecvError::Closed) => None,
        }
    }
}

#[cfg(test)]
mod tests;
