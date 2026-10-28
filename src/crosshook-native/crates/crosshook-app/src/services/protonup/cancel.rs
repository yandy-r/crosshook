//! Owner-checked cancellation of install jobs.

use super::messages::MSG_OP_NOT_FOUND;
use super::{InstallOwner, ProtonUpService};
use crate::{AppError, AppErrorKind};

impl ProtonUpService {
    /// Requests cancellation of the install `op_id` started by `owner`.
    ///
    /// Only ids in the owner ledger reach the job registry, so unknown ids,
    /// ids started by another owner and ids of unrelated jobs all fail the
    /// same way and nothing is cancelled. A successful request consumes the
    /// ledger entry, so cancelling the same op again is `NotFound`.
    ///
    /// # Errors
    /// `NotFound` ("install operation was not found") when `op_id` is not an
    /// active install of `owner`.
    pub fn cancel_install(&self, owner: &InstallOwner, op_id: &str) -> Result<(), AppError> {
        let id = self.owners.claim(owner, op_id)?;
        self.jobs
            .cancel(&id)
            .map_err(|_| AppError::new(AppErrorKind::NotFound, MSG_OP_NOT_FOUND))
    }
}
