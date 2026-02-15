use uuid::Uuid;
use crate::core::job_state::JobState;

pub enum JobEvent {
    StateChanged {
        id: Uuid,
        state: JobState,
    },
    Progress {
        id: Uuid,
        current: u64,
        total: u64,
    },
    Message {
        id: Uuid,
        text: String,
    },
}
