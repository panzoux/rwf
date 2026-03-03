use crate::failure_reason::FailureReason;
use crate::job_state::JobState;

#[derive(Debug)]
pub struct JobEvent {
    pub job_id: usize,
    pub state: JobState,
    pub message: Option<String>,
    pub failure: Option<FailureReason>,
}