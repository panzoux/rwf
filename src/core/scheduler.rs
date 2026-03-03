use crate::job::Job;
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct Scheduler {
    queue: Mutex<VecDeque<Job>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
        }
    }

    pub fn enqueue(&self, job: Job) {
        self.queue.lock().unwrap().push_back(job);
    }

    pub fn next(&self) -> Option<Job> {
        self.queue.lock().unwrap().pop_front()
    }
}