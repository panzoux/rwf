use crate::cancellation::CancellationToken;
use crate::event_bus::EventBus;
use crate::failure_reason::FailureReason;
use crate::job_event::JobEvent;
use crate::job_state::JobState;

use std::thread;
use std::time::Duration;

pub struct Job {
    pub id: usize,
    pub cancellation: CancellationToken,
}

impl Job {
    pub fn new(id: usize) -> Self {
        Self {
            id,
            cancellation: CancellationToken::new(),
        }
    }

    pub fn execute(self, event_sender: crossbeam_channel::Sender<JobEvent>) {
        event_sender
            .send(JobEvent {
                job_id: self.id,
                state: JobState::Running,
                message: Some("Started".into()),
                failure: None,
            })
            .ok();

        for _ in 0..5 {
            if self.cancellation.is_forced() {
                event_sender
                    .send(JobEvent {
                        job_id: self.id,
                        state: JobState::Failed,
                        message: Some("Force Cancelled".into()),
                        failure: Some(FailureReason::ForcedTermination),
                    })
                    .ok();
                return;
            }

            if self.cancellation.is_cancelled() {
                event_sender
                    .send(JobEvent {
                        job_id: self.id,
                        state: JobState::Cancelled,
                        message: Some("Cancelled".into()),
                        failure: None,
                    })
                    .ok();
                return;
            }

            thread::sleep(Duration::from_millis(300));
        }

        event_sender
            .send(JobEvent {
                job_id: self.id,
                state: JobState::Completed,
                message: Some("Completed".into()),
                failure: None,
            })
            .ok();
    }
}