use crate::scheduler::Scheduler;
use crate::job_event::JobEvent;
use std::sync::Arc;
use std::thread;

#[derive(Clone)]
pub struct WorkerPool {
    size: usize,
    scheduler: Arc<Scheduler>,
}

impl WorkerPool {
    pub fn new(size: usize, scheduler: Arc<Scheduler>) -> Self {
        Self { size, scheduler }
    }

    pub fn start(&self) {
        for _ in 0..self.size {
            let scheduler = self.scheduler.clone();
            thread::spawn(move || loop {
                if let Some(job) = scheduler.next() {
                    let (sender, _receiver) = crossbeam_channel::unbounded::<JobEvent>();
                    job.execute(sender);
                } else {
                    thread::sleep(std::time::Duration::from_millis(50));
                }
            });
        }
    }
}