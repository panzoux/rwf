use std::collections::{HashMap, VecDeque};
use uuid::Uuid;

use super::job::Job;
use super::job_state::JobState;
use super::lock_table::LockTable;

pub struct Scheduler {
    queue: VecDeque<Uuid>,
    jobs: HashMap<Uuid, Box<dyn Job>>,
    lock_table: LockTable,
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            jobs: HashMap::new(),
            lock_table: LockTable::new(),
        }
    }

    pub fn enqueue(&mut self, job: Box<dyn Job>) {
        let id = job.id();
        self.jobs.insert(id, job);
        self.queue.push_back(id);
    }

    pub fn tick(&mut self) {
        if let Some(job_id) = self.queue.pop_front() {
            if let Some(job) = self.jobs.get_mut(&job_id) {
                if self.dependencies_satisfied(job_id) {
                    // 実行開始は後で実装
                    println!("Starting job: {:?}", job_id);
                } else {
                    println!("Dependencies not satisfied for: {:?}", job_id);
                }
            }
        }
    }

    fn dependencies_satisfied(&self, job_id: Uuid) -> bool {
        if let Some(job) = self.jobs.get(&job_id) {
            for dep in job.dependencies() {
                if let Some(dep_job) = self.jobs.get(dep) {
                    match dep_job.state() {
                        JobState::Completed => continue,
                        JobState::Failed | JobState::Cancelled => return false,
                        _ => return false,
                    }
                }
            }
        }
        true
    }
}
