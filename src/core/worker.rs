use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::lock_table::LockTable;
use crate::scheduler::Scheduler;

pub struct WorkerPool {
    _handles: Vec<thread::JoinHandle<()>>,
}

impl WorkerPool {
    pub fn new(
        size: usize,
        scheduler: Arc<Scheduler>,
        lock_table: Arc<LockTable>,
    ) -> Self {
        let mut handles = Vec::new();

        for _ in 0..size {
            let scheduler = scheduler.clone();
            let lock_table = lock_table.clone();

            let handle = thread::spawn(move || loop {
                if let Some(job) = scheduler.dequeue() {
                    job.execute(lock_table.clone());
                } else {
                    thread::sleep(Duration::from_millis(50));
                }
            });

            handles.push(handle);
        }

        Self { _handles: handles }
    }
}