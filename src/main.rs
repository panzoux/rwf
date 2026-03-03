mod cancellation;
mod event_bus;
mod failure_reason;
mod job;
mod job_event;
mod job_state;
mod lock_table;
mod scheduler;
mod status_model;
mod ui_loop;
mod worker_pool;

use crate::event_bus::EventBus;
use crate::scheduler::Scheduler;
use crate::ui_loop::UiLoop;
use crate::worker_pool::WorkerPool;

use std::sync::Arc;

fn main() {
    let event_bus = EventBus::new();
    let scheduler = Arc::new(Scheduler::new());
    let worker_pool = WorkerPool::new(4, scheduler.clone());

    worker_pool.start();

    let mut ui = UiLoop::new(
        event_bus.receiver.clone(),
        scheduler.clone(),
        worker_pool.clone(),
    );

    ui.run();
}