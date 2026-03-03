# Event Receiver Integration Guide

This document explains how to integrate the event receiver into the UI thread's main event loop.

## Overview

The event receiver is responsible for:
1. Receiving JobEvents from the WorkerPool
2. Converting JobEvents to Transition enum values
3. Feeding transitions to the update_state function
4. Processing the resulting StateUpdateResult

## Architecture

```
┌─────────────┐
│ Worker Pool │
│   (Jobs)    │
└──────┬──────┘
       │ JobEvent
       ▼
┌─────────────────┐
│ Event Receiver  │
│ (UI Thread)     │
└──────┬──────────┘
       │ Transition
       ▼
┌─────────────────┐
│  update_state   │
│  (Pure Fn)      │
└──────┬──────────┘
       │ StateUpdateResult
       ▼
┌─────────────────┐
│   AppState      │
│   (Updated)     │
└─────────────────┘
```

## Basic Usage

### Non-Blocking Event Processing

Use `process_pending_events` in your main event loop to process all available events without blocking:

```rust
use rwf_lib::{AppState, WorkerPool, process_pending_events};

async fn main_event_loop() {
    let mut state = AppState::new(Default::default());
    let mut pool = WorkerPool::new(4);
    
    loop {
        // Process all pending job events
        let results = process_pending_events(&mut pool, &mut state);
        
        // Handle side effects from state updates
        for result in results {
            if result.ui_changed {
                // Trigger UI redraw
            }
            
            for job_spec in result.jobs_to_start {
                pool.submit_job(job_spec);
            }
            
            for job_id in result.jobs_to_cancel {
                // Handle job cancellation
            }
        }
        
        // Process user input
        // ...
        
        // Render UI
        // ...
        
        // Small delay to prevent busy-waiting
        tokio::time::sleep(tokio::time::Duration::from_millis(16)).await;
    }
}
```

### Blocking Event Processing

Use `process_next_event` when you want to wait for the next event:

```rust
use rwf_lib::{AppState, WorkerPool, process_next_event};

async fn event_processor() {
    let mut state = AppState::new(Default::default());
    let mut pool = WorkerPool::new(4);
    
    while let Some(result) = process_next_event(&mut pool, &mut state).await {
        // Handle the state update result
        if result.ui_changed {
            // Trigger UI redraw
        }
        
        for job_spec in result.jobs_to_start {
            pool.submit_job(job_spec);
        }
    }
}
```

### Manual Event Mapping

If you need more control, you can manually map events to transitions:

```rust
use rwf_lib::{WorkerPool, AppState, map_job_event_to_transition, update_state};

async fn custom_event_handler(pool: &mut WorkerPool, state: &mut AppState) {
    if let Some(event) = pool.try_recv_event() {
        // Convert event to transition
        let transition = map_job_event_to_transition(event);
        
        // Apply transition to state
        let result = update_state(state, transition);
        
        // Handle result
        // ...
    }
}
```

## Integration with UI Event Loop

Here's a complete example showing how to integrate event processing with user input and rendering:

```rust
use rwf_lib::{AppState, WorkerPool, process_pending_events, Transition, update_state};
use crossterm::event::{self, Event, KeyCode};
use std::time::Duration;

async fn main_loop() -> anyhow::Result<()> {
    let mut state = AppState::new(Default::default());
    let mut pool = WorkerPool::new(4);
    
    loop {
        // 1. Process job events (non-blocking)
        let job_results = process_pending_events(&mut pool, &mut state);
        let mut needs_redraw = false;
        
        for result in job_results {
            needs_redraw |= result.ui_changed;
            
            // Start any new jobs
            for job_spec in result.jobs_to_start {
                pool.submit_job(job_spec);
            }
        }
        
        // 2. Process user input (non-blocking)
        if event::poll(Duration::from_millis(0))? {
            if let Event::Key(key) = event::read()? {
                let transition = match key.code {
                    KeyCode::Tab => Transition::SwitchPane,
                    KeyCode::Up => Transition::CursorMove { 
                        pane: state.ui.active_pane, 
                        delta: -1 
                    },
                    KeyCode::Down => Transition::CursorMove { 
                        pane: state.ui.active_pane, 
                        delta: 1 
                    },
                    KeyCode::Char('q') => Transition::Quit,
                    _ => continue,
                };
                
                let result = update_state(&mut state, transition);
                needs_redraw |= result.ui_changed;
                
                // Start any new jobs from user actions
                for job_spec in result.jobs_to_start {
                    pool.submit_job(job_spec);
                }
                
                // Handle quit
                if matches!(transition, Transition::Quit) {
                    break;
                }
            }
        }
        
        // 3. Render UI if needed
        if needs_redraw {
            // render(&state)?;
        }
        
        // 4. Small delay to prevent busy-waiting (60 FPS)
        tokio::time::sleep(Duration::from_millis(16)).await;
    }
    
    // Cleanup
    pool.shutdown().await;
    
    Ok(())
}
```

## Event Flow

### JobEvent Types and Their Transitions

| JobEvent | Transition | Effect |
|----------|-----------|--------|
| `Started(job_id)` | `UpdateJobProgress { job_id, progress: 0.0 }` | Updates job progress to 0% |
| `Progress(job_id, progress)` | `UpdateJobProgress { job_id, progress }` | Updates job progress |
| `Completed(job_id, data)` | `CompleteJob { job_id, result: Success(data) }` | Marks job as completed successfully |
| `Failed(job_id, error)` | `CompleteJob { job_id, result: Failed(error) }` | Marks job as failed |
| `Cancelled(job_id)` | `AcknowledgeCancel { job_id }` | Acknowledges job cancellation |

## Best Practices

1. **Non-Blocking Processing**: Always use `process_pending_events` in the main UI loop to avoid blocking on job events.

2. **Handle Side Effects**: Always process the `StateUpdateResult` to start new jobs and handle cancellations.

3. **UI Responsiveness**: Keep the event loop running at 60 FPS (16ms per frame) to maintain UI responsiveness as per Requirement 21.4.

4. **Error Handling**: Wrap event processing in proper error handling to prevent crashes.

5. **Graceful Shutdown**: Always call `pool.shutdown().await` before exiting to ensure all workers complete cleanly.

## Requirements Validation

This implementation validates the following requirements:

- **21.1**: The UI_Thread SHALL never block on file I/O operations
  - ✓ `process_pending_events` uses `try_recv_event` which is non-blocking
  
- **21.5**: WHEN a Job is running, THE Application SHALL continue to accept user input
  - ✓ Event processing is non-blocking, allowing user input processing in the same loop
  
- **21.8**: THE Application SHALL receive job progress updates via JobEvent channel without blocking the UI_Thread
  - ✓ JobEvents are received via non-blocking channel operations
  
- **26.7**: THE Application SHALL process user input by mapping KeyEvent to Transition values
  - ✓ Both JobEvents and KeyEvents are mapped to Transitions and processed uniformly

## Testing

The event receiver includes comprehensive tests:

- `test_map_started_event`: Verifies Started events map to UpdateJobProgress
- `test_map_progress_event`: Verifies Progress events map correctly
- `test_map_completed_event`: Verifies Completed events map to CompleteJob
- `test_map_failed_event`: Verifies Failed events map to CompleteJob with error
- `test_map_cancelled_event`: Verifies Cancelled events map to AcknowledgeCancel
- `test_process_pending_events_empty`: Verifies no-op when no events available
- `test_process_pending_events_with_events`: Verifies multiple events are processed
- `test_process_next_event`: Verifies async event waiting

Run tests with:
```bash
cargo test --lib event_receiver
```
