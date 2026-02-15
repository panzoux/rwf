use uuid::Uuid;

use super::cancellation::CancellationHandle;
use super::failure_reason::FailureReason;
use super::job_state::JobState;

pub trait Job: Send {
    fn id(&self) -> Uuid;

    fn state(&self) -> JobState;

    fn start(&mut self, cancel: CancellationHandle) -> Result<(), FailureReason>;

    fn request_cancel(&mut self);

    fn force_cancel(&mut self);

    fn dependencies(&self) -> &[Uuid];
}
