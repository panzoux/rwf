# rwf_spec.md

## rwf

Version: 1.0 (Consolidated Specification) Status: Implementation-ready

------------------------------------------------------------------------

# 1. Purpose

rwf is a lightweight, event-driven, non-blocking file operation engine.

Primary goal:

-   Execute file operations asynchronously
-   Never block UI thread
-   Provide precise job state reporting
-   Support cooperative cancellation
-   Support force cancel (logical immediate termination)
-   Maintain strict architectural simplicity

rwf is NOT: - A batch system - A DAG scheduler - A distributed system -
A complex dependency resolver

It is a deterministic FIFO worker-based execution engine.

------------------------------------------------------------------------

# 2. Design Principles

1.  UI must never block
2.  Worker count fixed at startup (configurable)
3.  Scheduler is strictly FIFO
4.  LockTable is minimal (path-level exclusion only)
5.  Cancellation is cooperative
6.  Force Cancel transitions immediately to terminal state
7.  No AppState monolith
8.  Event-driven feedback loop

------------------------------------------------------------------------

# 3. Thread Model

UI Thread - Receives input - Submits jobs - Receives job events -
Renders state

Worker Pool - Fixed N threads - Each worker executes exactly one job at
a time - Workers pull from Scheduler (FIFO)

Event Channel - Workers send JobEvent to UI thread

------------------------------------------------------------------------

# 4. Architecture Diagram (Mermaid)

``` mermaid
flowchart TD
    UI[UI Thread]
    SCH[Scheduler FIFO]
    WP[Worker Pool]
    JOB[Job]
    FS[Filesystem]
    EVT[Event Channel]

    UI -->|submit| SCH
    SCH --> WP
    WP --> JOB
    JOB --> FS
    JOB --> EVT
    EVT --> UI
```

------------------------------------------------------------------------

# 5. Core Components

## 5.1 Job Trait

Responsibilities: - run() - check cancellation flag - report events -
return Result

Job lifecycle: Queued → Running → Completed / Failed / Cancelled

------------------------------------------------------------------------

## 5.2 JobState

enum JobState { Queued, Running, Completed, Failed(FailureReason),
Cancelled, }

No Cancelling state (accurate state representation required).

------------------------------------------------------------------------

## 5.3 Cancellation Model

Cooperative cancellation:

-   AtomicBool flag shared with job
-   Job checks periodically
-   On detection → return Cancelled

Force Cancel:

-   State immediately transitions to Cancelled
-   Worker stops observing further output
-   No thread killing
-   Cleanup responsibility remains in Job

------------------------------------------------------------------------

## 5.4 Scheduler

Strict FIFO queue.

No: - Priority - Dependency graph - Rescheduling - Work stealing

------------------------------------------------------------------------

## 5.5 Worker Pool

-   Configurable at startup
-   Default: 4 workers
-   Fixed after initialization
-   Workers block on queue receive

------------------------------------------------------------------------

## 5.6 LockTable

Minimal path-based locking.

Purpose: - Prevent concurrent operations on same path - No hierarchical
locking - No DAG resolution

------------------------------------------------------------------------

# 6. Event Model

enum JobEvent { Started(JobId), Progress(JobId, u64), Completed(JobId),
Failed(JobId, FailureReason), Cancelled(JobId), }

UI processes events in receiver loop.

------------------------------------------------------------------------

# 7. Failure Model

enum FailureReason { IoError(String), PermissionDenied,
InvalidOperation, Unknown, }

Failures are terminal states.

------------------------------------------------------------------------

# 8. Crate Dependencies

Runtime: - tokio = { version = "1", features = \["rt-multi-thread",
"macros"\] }

Concurrency: - crossbeam-channel = "0.5" - parking_lot = "0.12"

Filesystem: - walkdir = "2" - fs_extra = "1.3"

Error Handling: - thiserror = "1" - anyhow = "1"

Logging: - tracing = "0.1" - tracing-subscriber = "0.3"

Optional (future): - notify = "6" (filesystem watch)

------------------------------------------------------------------------

# 9. Directory Structure

src/ main.rs scheduler.rs worker_pool.rs job.rs job_state.rs
job_event.rs cancellation.rs failure_reason.rs lock_table.rs

------------------------------------------------------------------------

# 10. Execution Flow

1.  UI submits Job
2.  Scheduler pushes to FIFO
3.  Worker picks job
4.  Job state → Running
5.  Job executes filesystem I/O
6.  Job sends events
7.  Job completes → terminal state
8.  UI updates display

------------------------------------------------------------------------

# 11. Non-Goals

-   Dynamic worker resizing
-   Priority scheduling
-   Distributed execution
-   Complex dependency resolution
-   Transaction system

------------------------------------------------------------------------

# 12. Determinism Guarantees

-   FIFO ordering respected
-   Max concurrency bounded by worker count
-   State transitions strictly linear
-   No hidden background mutation

------------------------------------------------------------------------

# 13. Ready for Implementation

This specification is self-contained and suitable for automated
plan-driven implementation.
