# Worker Pool Integration Guide

## Overview

The Worker Pool is a core component of the two-pane file manager that enables asynchronous execution of file operations without blocking the UI thread. It follows the Reactive Worker Framework (rwf) pattern with strict FIFO job ordering.

## Architecture

### Components

1. **WorkerPool**: Manages a fixed number of worker threads
2. **JobEvent**: Events sent from workers to the UI thread
3. **JobSpec**: Job specifications submitted to the pool
4. **JobManager**: Manages job queue and state (in AppState)

### Thread Model

```
┌─────────────┐
│  UI Thread  │
│             │
│  - Input    │
│  - Render   │
│  - Events   │
└──────┬──────┘
       │
       │ submit_job()
       ▼
┌─────────────────────┐
│   Worker Pool       │
│                     │
│  ┌────┐  ┌────┐    │
│  │ W1 │  │ W2 │    │
│  └────┘  └────┘    │
│  ┌────┐  ┌────┐    │
│  │ W3 │  │ W4 │    │
│  └────┘  └────┘    │
└──────┬──────────────┘
       │
       │ JobEvent
       ▼
┌─────────────┐
│  UI Thread  │
│  (Events)   │
└─────────────┘
```

## Configuration

The worker pool size is configurable via `AppConfig`:

```rust
use rwf_lib::{AppState, AppConfig};

let config = AppConfig {
    worker_pool_size: 4, // Default: 4 workers
};

let state = AppState::new(config);
```

## Job Submission

Jobs are submitted through the `WorkerPool::submit_job()` method:

```rust
use rwf_lib::{WorkerPool, JobSpec, JobKind};
use rwf_lib::model::Location;
use std::path::PathBuf;

let pool = WorkerPool::new(4);

// Create a job specification
let job = JobSpec::new(JobKind::ReadDirectory {
    location: Location::Local(PathBuf::from("/home/user")),
});

// Submit to worker pool
pool.submit_job(job);
```

## Event Processing

The worker pool sends events back to the UI thread via an unbounded channel:

```rust
use rwf_lib::JobEvent;

// In the UI event loop
while let Some(event) = pool.recv_event().await {
    match event {
        JobEvent::Started(job_id) => {
            println!("Job {:?} started", job_id);
        }
        JobEvent::Progress(job_id, progress) => {
            println!("Job {:?}: {:.1}%", job_id, progress * 100.0);
        }
        JobEvent::Completed(job_id, data) => {
            println!("Job {:?} completed", job_id);
            // Update UI with results
        }
        JobEvent::Failed(job_id, error) => {
            eprintln!("Job {:?} failed: {}", job_id, error);
        }
        JobEvent::Cancelled(job_id) => {
            println!("Job {:?} cancelled", job_id);
        }
    }
}
```

## Integration with AppState

The worker pool works in conjunction with the `JobManager` in `AppState`:

```rust
use rwf_lib::{AppState, Transition, StateUpdateResult};

let mut state = AppState::new(AppConfig::default());
let mut pool = WorkerPool::new(state.config.worker_pool_size);

// State transitions can create jobs
let result = update_state(&mut state, Transition::ChangeLocation {
    pane: ActivePane::Left,
    location: Location::Local(PathBuf::from("/tmp")),
});

// Submit jobs from the result
for job_spec in result.jobs_to_start {
    pool.submit_job(job_spec);
}
```

## FIFO Ordering

The worker pool maintains strict FIFO (first-in-first-out) ordering:

- Jobs are executed in the order they are submitted
- Multiple workers pull from a shared queue
- No priority scheduling or reordering

## Cooperative Cancellation

Jobs support cooperative cancellation via `CancellationToken`:

```rust
// Request cancellation
let result = update_state(&mut state, Transition::CancelJob { job_id });

// The job checks the cancellation token periodically
// and sends a Cancelled event when it acknowledges
```

## Shutdown

The worker pool can be gracefully shut down:

```rust
// Drop the job sender and wait for workers to finish
pool.shutdown().await;
```

## Requirements Satisfied

This implementation satisfies the following requirements:

- **17.8**: Worker Pool size configuration (default: 4 workers)
- **20.2**: Initialize Worker Pool with configured worker count
- **21.1**: All file I/O operations execute as Jobs (never block UI thread)
- **21.7**: Worker Pool executes all file operations asynchronously

## Next Steps

The following tasks build on this foundation:

- **Task 8.2**: Implement JobEvent enum (already done in this task)
- **Task 8.3**: Implement event receiver on UI thread
- **Task 9**: Implement FilesystemBackend and LocalFilesystemBackend
- **Task 10**: Implement JobExecutor for actual job execution

## Example Usage

See `examples/worker_pool_demo.rs` for a complete working example.
