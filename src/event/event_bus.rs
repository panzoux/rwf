use crossbeam_channel::{unbounded, Receiver, Sender};
use crate::job_event::JobEvent;

pub struct EventBus {
    pub sender: Sender<JobEvent>,
    pub receiver: Receiver<JobEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (s, r) = unbounded();
        Self { sender: s, receiver: r }
    }
}