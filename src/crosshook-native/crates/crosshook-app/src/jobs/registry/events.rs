//! Job handles and per-job event receivers.

use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::sync::CancellationToken;

use crate::jobs::types::{JobEvent, JobEventKind, JobId};
use crate::log_tail::LogBatch;

/// Returned by [`JobRegistry::start`]: the job id, its cancel token, and a
/// receiver subscribed before the job began.
pub struct JobHandle {
    pub(super) id: JobId,
    pub(super) cancel: CancellationToken,
    pub(super) events: JobEvents,
}

impl JobHandle {
    /// The job id.
    pub fn id(&self) -> &JobId {
        &self.id
    }

    /// The job's cancellation token.
    pub fn cancel_token(&self) -> &CancellationToken {
        &self.cancel
    }

    /// Requests cancellation of this job only.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Receives the next event; see [`JobEvents::recv`].
    pub async fn recv(&mut self) -> Option<JobEvent> {
        self.events.recv().await
    }

    /// Splits off the event receiver.
    pub fn into_events(self) -> JobEvents {
        self.events
    }
}

enum Source {
    Live(broadcast::Receiver<JobEvent>),
    Replay(Option<JobEvent>),
}

/// Receiver of one job's events.
pub struct JobEvents {
    id: JobId,
    source: Source,
    done: bool,
}

impl JobEvents {
    pub(super) fn live(id: JobId, rx: broadcast::Receiver<JobEvent>) -> Self {
        Self {
            id,
            source: Source::Live(rx),
            done: false,
        }
    }

    pub(super) fn replay(finished: JobEvent) -> Self {
        Self {
            id: finished.job_id.clone(),
            source: Source::Replay(Some(finished)),
            done: false,
        }
    }

    /// The job these events belong to.
    pub fn job_id(&self) -> &JobId {
        &self.id
    }

    /// Next event, or `None` after `Finished` was yielded. A lagging receiver
    /// gets `Log(LogBatch::Dropped { n })` with the number of lost events,
    /// then the retained events; `Finished` is never lost to lag.
    pub async fn recv(&mut self) -> Option<JobEvent> {
        if self.done {
            return None;
        }
        let event = match &mut self.source {
            Source::Replay(finished) => finished.take(),
            Source::Live(rx) => match rx.recv().await {
                Ok(event) => Some(event),
                Err(RecvError::Lagged(n)) => Some(JobEvent {
                    job_id: self.id.clone(),
                    kind: JobEventKind::Log(LogBatch::Dropped { n }),
                }),
                Err(RecvError::Closed) => None,
            },
        };
        match &event {
            None
            | Some(JobEvent {
                kind: JobEventKind::Finished(_),
                ..
            }) => self.done = true,
            Some(_) => {}
        }
        event
    }
}
