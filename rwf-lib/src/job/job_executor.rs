//! Job executor for processing JobSpecs
//!
//! This module implements the JobExecutor that dispatches jobs to the
//! appropriate backend methods and sends JobEvent updates.

use crate::backend::{FilesystemBackend, ArchiveHandler};
use crate::job::{JobSpec, JobId, JobKind, OpResult, SuccessData, PipeToAction};
use crate::model::Location;
use crate::model::viewer::{FileBytes, LineIndex, ViewerBuffer, TextEncoding};
use crate::worker_pool::JobEvent;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;
use tracing::debug;

/// Executor for processing job specifications
pub struct JobExecutor<B: FilesystemBackend, A: ArchiveHandler> {
    backend: Arc<B>,
    archive_handler: Arc<A>,
    event_sender: mpsc::UnboundedSender<JobEvent>,
}

impl<B: FilesystemBackend, A: ArchiveHandler> JobExecutor<B, A> {
    /// Create a new JobExecutor
    ///
    /// # Arguments
    ///
    /// * `backend` - The filesystem backend to use for operations
    /// * `archive_handler` - The archive handler for archive operations
    /// * `event_sender` - Channel for sending job events to the UI thread
    pub fn new(
        backend: Arc<B>,
        archive_handler: Arc<A>,
        event_sender: mpsc::UnboundedSender<JobEvent>,
    ) -> Self {
        Self {
            backend,
            archive_handler,
            event_sender,
        }
    }
    
    /// Execute a job specification
    ///
    /// Dispatches to the appropriate backend method based on JobKind
    /// and sends JobEvent updates via the event channel.
    pub async fn execute(&self, spec: JobSpec) {
        let job_id = spec.id;
        
        // Always send Started event first
        if let Err(e) = self.event_sender.send(JobEvent::Started(job_id)) {
            tracing::error!("Failed to send job started event for {:?}: {}", job_id, e);
        }
        
        let result = match &spec.kind {
            JobKind::ReadDirectory { location } => {
                self.execute_read_directory(location, &spec.cancel_token).await
            }
            JobKind::Copy { sources, dest } => {
                self.execute_copy(sources, dest, &spec).await
            }
            JobKind::Move { sources, dest } => {
                self.execute_move(sources, dest, &spec).await
            }
            JobKind::Delete { targets } => {
                self.execute_delete(targets, &spec).await
            }
            JobKind::Mkdir { location } => {
                self.execute_mkdir(location, &spec.cancel_token).await
            }
            JobKind::Rename { from, to } => {
                self.execute_rename(from, to, &spec.cancel_token).await
            }
            JobKind::CalculateSize { location } => {
                self.execute_calculate_size(location, &spec).await
            }
            JobKind::ExtractArchive { archive, dest } => {
                self.execute_extract_archive(archive, dest, &spec).await
            }
            JobKind::CreateArchive { sources, dest, original_size: _ } => {
                self.execute_create_archive(sources, dest, &spec).await
            }
            JobKind::ExecuteCustomFunction { command, working_dir, pipe_to_action, shell } => {
                self.execute_custom_function(command, working_dir, pipe_to_action, &spec, shell.as_deref()).await
            }
            JobKind::SpawnProcess { program, args } => {
                self.execute_spawn_process(program, args, &spec).await
            }
            JobKind::Search { location, pattern, recursive } => {
                self.execute_search(location, pattern, *recursive, &spec).await
            }
            JobKind::LoadFileForViewer { location, index_lines } => {
                self.execute_load_file_for_viewer(job_id, location, *index_lines, &spec.cancel_token).await
            }
            JobKind::PatternRename { targets, find, replace, use_regex, case_sensitive } => {
                self.execute_pattern_rename(targets, find, replace, *use_regex, *case_sensitive, &spec).await
            }
            JobKind::CompareFiles { left, right } => {
                self.execute_compare_files(left, right, &spec).await
            }
            JobKind::SplitFile { source, dest_dir, chunk_size } => {
                self.execute_split_file(source, dest_dir, *chunk_size, &spec).await
            }
            JobKind::JoinFiles { parts, dest } => {
                self.execute_join_files(parts, dest, &spec).await
            }
            JobKind::CountDown { duration_secs, start_value } => {
                self.execute_countdown(*duration_secs, *start_value, &spec).await
            }
            JobKind::CollectJumpCandidates { root, include_files, max_results, max_depth } => {
                self.execute_collect_jump_candidates(root, *include_files, *max_results, *max_depth, &spec.cancel_token).await
            }
        };
        
        // Send completion event based on result
        let event = match result {
            OpResult::Success(data) => JobEvent::Completed(job_id, data),
            OpResult::Failed(error) => JobEvent::Failed(job_id, error),
            OpResult::Cancelled => JobEvent::Cancelled(job_id),
        };
        
        if let Err(e) = self.event_sender.send(event) {
            tracing::trace!("Failed to send job completion event (receiver likely closed): {}", e);
        }
    }
    
    /// Execute a read directory operation
    async fn execute_read_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Check if this is an archive location
        match location {
            Location::Archive { .. } => {
                // Use archive handler for archive locations
                match self.archive_handler.list_entries(location, cancel_token).await {
                    Ok(entries) => OpResult::Success(SuccessData::DirectoryRead(entries)),
                    Err(e) => OpResult::Failed(e.to_string()),
                }
            }
            _ => {
                // Use regular backend for other locations
                match self.backend.read_directory(location, cancel_token).await {
                    Ok(entries) => OpResult::Success(SuccessData::DirectoryRead(entries)),
                    Err(e) => OpResult::Failed(e.to_string()),
                }
            }
        }
    }
    
    /// Execute a copy operation
    async fn execute_copy(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        let total_files = sources.len();
        let decisions = spec.conflict_decisions.as_ref();

        for (i, source) in sources.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Check if there's a conflict decision for this file
            let decision = decisions.and_then(|d| d.iter().find(|d| d.source == *source));
            
            // Handle decision
            if let Some(dec) = decision {
                match &dec.action {
                    crate::job::ConflictAction::Skip => {
                        debug!("Skipping file due to conflict decision: {:?}", source);
                        continue;
                    }
                    crate::job::ConflictAction::OverwriteIfNewer => {
                        // Check if source is newer
                        let dest_location = if let Some(filename) = self.get_filename(source) {
                            self.join_location(dest, &filename)
                        } else {
                            dest.clone()
                        };
                        
                        if let (Ok(src_meta), Ok(dst_meta)) = (
                            self.backend.get_entry(source).await,
                            self.backend.get_entry(&dest_location).await,
                        ) {
                            if src_meta.modified <= dst_meta.modified {
                                debug!("Skipping file (not newer): {:?}", source);
                                continue;
                            }
                        }
                    }
                    crate::job::ConflictAction::Rename { new_name } => {
                        // Copy with new name
                        if let Some(dest_path) = dest.path() {
                            let new_dest = Location::Local(dest_path.join(new_name));
                            if let Err(e) = self.backend.copy_file(source, &new_dest, &spec.cancel_token).await {
                                return OpResult::Failed(format!("Failed to copy {} as {}: {}",
                                    self.location_display(source), new_name, e));
                            }
                            debug!("Renamed and copied file: {} -> {}", self.location_display(source), new_name);
                            
                            // Progress update
                            let progress = if total_files > 0 { (i + 1) as f64 / total_files as f64 } else { 1.0 };
                                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
                            continue;
                        }
                    }
                    crate::job::ConflictAction::Force => {
                        // Proceed with normal copy (overwrite)
                    }
                }
            }

            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };

            // Send progress update
                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }

            // Determine destination path
            let dest_location = if let Some(filename) = self.get_filename(source) {
                self.join_location(dest, &filename)
            } else {
                dest.clone()
            };

            // Copy the file
            if let Err(e) = self.backend.copy_file(source, &dest_location, &spec.cancel_token).await {
                return OpResult::Failed(format!("Failed to copy {}: {}",
                    self.location_display(source), e));
            }
        }

        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!("Failed to send job progress event (1.0) for {:?}: {}", spec.id, e);
        }

        OpResult::Success(SuccessData::None)
    }
    
    /// Execute a move operation
    async fn execute_move(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        let total_files = sources.len();
        let decisions = spec.conflict_decisions.as_ref();

        for (i, source) in sources.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Check if there's a conflict decision for this file
            let decision = decisions.and_then(|d| d.iter().find(|d| d.source == *source));
            
            // Handle decision
            if let Some(dec) = decision {
                match &dec.action {
                    crate::job::ConflictAction::Skip => {
                        debug!("Skipping file due to conflict decision: {:?}", source);
                        continue;
                    }
                    crate::job::ConflictAction::OverwriteIfNewer => {
                        // Check if source is newer
                        let dest_location = if let Some(filename) = self.get_filename(source) {
                            self.join_location(dest, &filename)
                        } else {
                            dest.clone()
                        };
                        
                        if let (Ok(src_meta), Ok(dst_meta)) = (
                            self.backend.get_entry(source).await,
                            self.backend.get_entry(&dest_location).await,
                        ) {
                            if src_meta.modified <= dst_meta.modified {
                                debug!("Skipping file (not newer): {:?}", source);
                                continue;
                            }
                        }
                    }
                    crate::job::ConflictAction::Rename { new_name } => {
                        // Move with new name
                        if let Some(dest_path) = dest.path() {
                            let new_dest = Location::Local(dest_path.join(new_name));
                            if let Err(e) = self.backend.move_file(source, &new_dest, &spec.cancel_token).await {
                                return OpResult::Failed(format!("Failed to move {} as {}: {}",
                                    self.location_display(source), new_name, e));
                            }
                            debug!("Renamed and moved file: {} -> {}", self.location_display(source), new_name);
                            
                            // Progress update
                            let progress = if total_files > 0 { (i + 1) as f64 / total_files as f64 } else { 1.0 };
                                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
                            continue;
                        }
                    }
                    crate::job::ConflictAction::Force => {
                        // Proceed with normal move (overwrite)
                    }
                }
            }

            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };

            // Send progress update
                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }

            // Determine destination path
            let dest_location = if let Some(filename) = self.get_filename(source) {
                self.join_location(dest, &filename)
            } else {
                dest.clone()
            };

            // Move the file
            if let Err(e) = self.backend.move_file(source, &dest_location, &spec.cancel_token).await {
                return OpResult::Failed(format!("Failed to move {}: {}",
                    self.location_display(source), e));
            }
        }

        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!("Failed to send job progress event (1.0) for {:?}: {}", spec.id, e);
        }

        OpResult::Success(SuccessData::None)
    }
    
    /// Execute a delete operation
    async fn execute_delete(
        &self,
        targets: &[Location],
        spec: &JobSpec,
    ) -> OpResult {
        let total_files = targets.len();
        
        for (i, target) in targets.iter().enumerate() {
            // Check cancellation before each file
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }
            
            // Calculate progress
            let progress = if total_files > 0 {
                (i as f64) / (total_files as f64)
            } else {
                0.0
            };
            
            // Send progress update
                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
            
            // Delete the file
            if let Err(e) = self.backend.delete_file(target, &spec.cancel_token).await {
                return OpResult::Failed(format!("Failed to delete {}: {}", 
                    self.location_display(target), e));
            }
        }
        
        // Send final progress
        if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, 1.0)) {
            tracing::error!("Failed to send job progress event (1.0) for {:?}: {}", spec.id, e);
        }
        
        OpResult::Success(SuccessData::None)
    }
    
    /// Execute a mkdir operation
    async fn execute_mkdir(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        match self.backend.create_directory(location, cancel_token).await {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }
    
    /// Execute a rename operation
    async fn execute_rename(
        &self,
        from: &Location,
        to: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        match self.backend.rename_file(from, to, cancel_token).await {
            Ok(_) => OpResult::Success(SuccessData::None),
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }
    
    /// Execute a calculate size operation
    async fn execute_calculate_size(
        &self,
        location: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        // Use the progress callback version to send updates
        let event_sender = self.event_sender.clone();
        let job_id = spec.id;
        
        // Track progress updates
        let last_update = std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now()));
        
        let result = self.backend.calculate_directory_size_with_progress(
            location,
            &spec.cancel_token,
            Box::new(move |items_processed, _current_size| {
                // Send progress updates every 100ms to avoid flooding
                let mut last = last_update.lock().unwrap();
                if last.elapsed() > std::time::Duration::from_millis(100) {
                    // We don't know the total, so we can't calculate a percentage
                    // Send a progress value that indicates activity (oscillating between 0.3 and 0.7)
                    let progress = 0.5 + 0.2 * ((items_processed % 10) as f64 / 10.0 - 0.5);
                    if let Err(e) = event_sender.send(JobEvent::Progress(job_id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", job_id, e);
                    }
                    *last = std::time::Instant::now();
                }
            })
        ).await;
        
        match result {
            Ok(size) => OpResult::Success(SuccessData::SizeCalculated(size)),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }
    
    /// Execute an extract archive operation
    async fn execute_extract_archive(
        &self,
        archive: &Location,
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        match self.archive_handler.extract_all(archive, dest, &spec.cancel_token).await {
            Ok(()) => OpResult::Success(SuccessData::None),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }
    
    /// Execute a create archive operation
    async fn execute_create_archive(
        &self,
        sources: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        match self.archive_handler.create_archive(sources, dest, &spec.cancel_token).await {
            Ok(()) => OpResult::Success(SuccessData::None),
            Err(e) => {
                if spec.cancel_token.is_cancelled() {
                    OpResult::Cancelled
                } else {
                    OpResult::Failed(e.to_string())
                }
            }
        }
    }
    
    /// Execute a custom function
    async fn execute_custom_function(
        &self,
        command: &str,
        working_dir: &Location,
        pipe_to_action: &Option<PipeToAction>,
        spec: &JobSpec,
        shell: Option<&str>,
    ) -> OpResult {
        // Extract local path from working directory
        let working_path = match working_dir {
            Location::Local(path) => path.clone(),
            _ => return OpResult::Failed("Custom functions only support local paths".to_string()),
        };
        
        // Check cancellation before starting
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Determine shell to use.
        // On Windows, cmd.exe gets /D before /C to disable AutoRun registry entries
        // (Clink and similar tools hook there; without /D they try to inject into the
        // non-interactive session and may return a non-zero exit code).
        #[derive(PartialEq)]
        enum ShellKind { Cmd, Other }
        let (shell_cmd, shell_arg, shell_kind) = if let Some(shell_name) = shell {
            match shell_name {
                "bash" => ("bash", "-c", ShellKind::Other),
                "zsh"  => ("zsh",  "-c", ShellKind::Other),
                "powershell" | "powershell.exe" => ("powershell", "-Command", ShellKind::Other),
                "cmd"  | "cmd.exe" => ("cmd", "/C", ShellKind::Cmd),
                _ => {
                    #[cfg(target_os = "windows")]
                    { ("cmd", "/C", ShellKind::Cmd) }
                    #[cfg(not(target_os = "windows"))]
                    { ("sh", "-c", ShellKind::Other) }
                }
            }
        } else {
            #[cfg(target_os = "windows")]
            { ("cmd", "/C", ShellKind::Cmd) }
            #[cfg(not(target_os = "windows"))]
            { ("sh", "-c", ShellKind::Other) }
        };

        // Execute the command.
        // For cmd.exe on Windows we use raw_arg instead of arg so that Rust does NOT
        // apply its own \"-escaping.  cmd.exe has its own quoting rules and treats
        // backslash-quote as two literal characters, corrupting paths that contain
        // double-quoted strings.  raw_arg passes the token exactly as given.
        let mut cmd = tokio::process::Command::new(shell_cmd);
        #[cfg(target_os = "windows")]
        {
            if shell_kind == ShellKind::Cmd {
                cmd.raw_arg("/D").raw_arg("/C").raw_arg(command);
            } else {
                cmd.arg(shell_arg).arg(command);
            }
        }
        #[cfg(not(target_os = "windows"))]
        cmd.arg(shell_arg).arg(command);
        cmd.current_dir(working_path);
        let output = cmd.output().await;
        
        match output {
            Ok(output) => {
                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                    
                    // Handle PipeToAction if specified
                    if let Some(_action) = pipe_to_action {
                        // PipeToAction handling will be done by the caller
                        // We just return the output
                        OpResult::Success(SuccessData::CustomFunctionOutput(stdout))
                    } else {
                        OpResult::Success(SuccessData::CustomFunctionOutput(stdout))
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    OpResult::Failed(stderr)
                }
            }
            Err(e) => OpResult::Failed(e.to_string()),
        }
    }
    
    /// Spawn a program directly without a shell, avoiding cmd.exe quote-mangling on Windows.
    async fn execute_spawn_process(&self, program: &str, args: &[String], spec: &JobSpec) -> crate::job::OpResult {
        if spec.cancel_token.is_cancelled() {
            return crate::job::OpResult::Cancelled;
        }
        match tokio::process::Command::new(program).args(args).spawn() {
            Ok(_) => crate::job::OpResult::Success(crate::job::SuccessData::None),
            Err(e) => crate::job::OpResult::Failed(e.to_string()),
        }
    }

    /// Execute a search operation
    async fn execute_search(
        &self,
        _location: &Location,
        _pattern: &str,
        _recursive: bool,
        _spec: &JobSpec,
    ) -> OpResult {
        // Search operations are not yet implemented
        // This is a placeholder for future implementation
        OpResult::Failed("Search not yet implemented".to_string())
    }
    
    /// Read the entire file into memory on the blocking pool and send ViewerReady.
    /// Using InMemory for all files means the UI thread (Tokio async worker) never
    /// touches OS file handles or mmap pages — eliminating page-fault stalls that
    /// occur when another process is concurrently writing to the file.
    async fn execute_load_file_for_viewer(
        &self,
        job_id: JobId,
        location: &Location,
        index_lines: bool,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }

        let path = match location {
            Location::Local(p) => p.clone(),
            Location::Archive { .. } =>
                return OpResult::Failed("Archive file viewing not yet implemented".to_string()),
            _ =>
                return OpResult::Failed("Unsupported location type for file viewing".to_string()),
        };

        // All file I/O runs on the blocking thread pool so the Tokio async thread
        // (which drives the UI event loop) is never stalled.
        //
        // Files ≤ INMEM_THRESHOLD are read entirely into RAM.  This avoids mmap
        // page-fault delays that occur when another process (e.g. clink) is
        // concurrently appending to the same file — each page fault can stall the
        // Tokio thread and make the viewer unresponsive.  The complete line index is
        // also built in the same blocking task so ViewerReady arrives with a fully
        // indexed, stable snapshot.
        //
        // Files > INMEM_THRESHOLD are memory-mapped (large log files, binaries) and
        // the newline index is built in 4 MB chunks on the blocking pool.
        const INMEM_THRESHOLD: usize = 100 * 1024 * 1024; // 100 MB

        let path_for_open = path.clone();
        let index_lines_flag = index_lines;

        // Returns (FileBytes, encoding, Option<complete LineIndex>).
        // The Option is Some for the InMemory path (index built inline).
        let open_result: std::io::Result<(FileBytes, TextEncoding, Option<LineIndex>)> =
            tokio::task::spawn_blocking(move || {
                let meta = std::fs::metadata(&path_for_open)?;
                let file_size = meta.len() as usize;

                if file_size <= INMEM_THRESHOLD {
                    let bytes = std::fs::read(&path_for_open)?;
                    let sample_len = bytes.len().min(16384);
                    let encoding = TextEncoding::detect(&bytes[..sample_len]);

                    let complete_index = if index_lines_flag {
                        let total = bytes.len();
                        let mut offsets: Vec<u64> = vec![0];
                        for (i, &b) in bytes.iter().enumerate() {
                            if b == b'\n' && i + 1 < total {
                                offsets.push((i + 1) as u64);
                            }
                        }
                        Some(LineIndex { offsets, is_complete: true })
                    } else {
                        None
                    };

                    Ok((FileBytes::InMemory(bytes), encoding, complete_index))
                } else {
                    // Large file: memory-map.
                    let file = std::fs::File::open(&path_for_open)?;
                    // SAFETY: Mmap holds its own OS mapping handle; dropping `file` is safe.
                    let mmap = unsafe { memmap2::Mmap::map(&file) }?;
                    let sample_len = mmap.len().min(16384);
                    let encoding = TextEncoding::detect(&mmap[..sample_len]);
                    Ok((FileBytes::Mapped(mmap), encoding, None))
                }
            })
            .await
            .unwrap_or_else(|e| Err(std::io::Error::other(e.to_string())));

        let (file_bytes, encoding, complete_index) = match open_result {
            Ok(triple) => triple,
            Err(e) => return OpResult::Failed(format!("Failed to open file: {}", e)),
        };

        // Small text-mode files arrive with a complete index, and hex mode needs none.
        // In both cases we can send ViewerReady immediately and return.
        if !index_lines || complete_index.is_some() {
            let idx = complete_index.unwrap_or_else(|| {
                let mut i = LineIndex::new(); i.is_complete = true; i
            });
            let buffer = ViewerBuffer::new(file_bytes, idx);
            let _ = self.event_sender.send(JobEvent::ViewerReady(job_id, buffer, encoding));
            return OpResult::Success(SuccessData::None);
        }

        // Large text-mode file: send the buffer first so the visible viewport renders
        // before the full index is ready, then build the index in 4 MB chunks on the
        // blocking pool.
        let buffer = ViewerBuffer::new(file_bytes, LineIndex::new());
        let _ = self.event_sender.send(JobEvent::ViewerReady(job_id, buffer.clone(), encoding));

        let total = buffer.total_bytes();
        if total == 0 {
            buffer.line_index.lock().unwrap().is_complete = true;
            return OpResult::Success(SuccessData::None);
        }

        let buffer_for_scan = buffer.clone();
        let cancel = cancel_token.clone();
        let event_tx = self.event_sender.clone();
        tokio::task::spawn_blocking(move || {
            const CHUNK: usize = 4 * 1024 * 1024;
            let bytes = buffer_for_scan.bytes.as_bytes();
            let mut chunk_start = 0usize;

            while chunk_start < total {
                if cancel.is_cancelled() { return; }

                let chunk_end = (chunk_start + CHUNK).min(total);
                let mut local: Vec<u64> = Vec::new();
                for i in chunk_start..chunk_end {
                    if bytes[i] == b'\n' && i + 1 < total {
                        local.push((i + 1) as u64);
                    }
                }
                if !local.is_empty() {
                    buffer_for_scan.line_index.lock().unwrap().offsets.extend_from_slice(&local);
                }
                let _ = event_tx.send(JobEvent::Progress(
                    job_id, chunk_end as f64 / total as f64,
                ));
                chunk_start = chunk_end;
            }
            buffer_for_scan.line_index.lock().unwrap().is_complete = true;
        })
        .await
        .ok();

        OpResult::Success(SuccessData::None)
    }
    
    /// Execute a pattern rename operation
    async fn execute_pattern_rename(
        &self,
        targets: &[Location],
        find: &str,
        replace: &str,
        use_regex: bool,
        case_sensitive: bool,
        spec: &JobSpec,
    ) -> OpResult {
        let total = targets.len();

        for (index, location) in targets.iter().enumerate() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }

            // Get the current filename
            let current_name = match self.get_filename(location) {
                Some(name) => name,
                None => {
                    return OpResult::Failed(format!(
                        "Could not extract filename from location: {}",
                        self.location_display(location)
                    ));
                }
            };

            // Apply the pattern to get the new name
            let new_name = crate::pattern_rename::apply_rename_pattern(
                &current_name, find, replace, use_regex, case_sensitive,
            );

            // Skip if the name hasn't changed
            if new_name == current_name {
                continue;
            }
            
            // Get the parent location and create the new location
            let new_location = match location.parent() {
                Some(parent) => parent.join(&new_name),
                None => {
                    return OpResult::Failed(format!(
                        "Could not determine parent directory for: {}",
                        self.location_display(location)
                    ));
                }
            };
            
            // Perform the rename
            match self.backend.rename_file(location, &new_location, &spec.cancel_token).await {
                Ok(_) => {
                    // Report progress
                    let progress = (index + 1) as f64 / total as f64;
                            if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
                }
                Err(e) => {
                    return OpResult::Failed(format!(
                        "Failed to rename {} to {}: {}",
                        current_name,
                        new_name,
                        e
                    ));
                }
            }
        }
        
        OpResult::Success(SuccessData::None)
    }
    
    async fn execute_compare_files(
        &self,
        left: &Location,
        right: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Read both files
        let left_contents = match self.read_file_as_string(left).await {
            Ok(contents) => contents,
            Err(e) => return OpResult::Failed(format!("Failed to read left file: {}", e)),
        };
        
        let right_contents = match self.read_file_as_string(right).await {
            Ok(contents) => contents,
            Err(e) => return OpResult::Failed(format!("Failed to read right file: {}", e)),
        };
        
        // Check for cancellation after reading files
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Split into lines
        let left_lines: Vec<String> = left_contents.lines().map(|s| s.to_string()).collect();
        let right_lines: Vec<String> = right_contents.lines().map(|s| s.to_string()).collect();
        
        // Perform simple line-by-line comparison
        let differences = self.compute_diff(&left_lines, &right_lines);
        
        let diff = crate::job::FileDiff {
            left_path: self.location_display(left),
            right_path: self.location_display(right),
            differences,
        };
        
        OpResult::Success(SuccessData::ComparisonResult(diff))
    }
    
    async fn execute_split_file(
        &self,
        source: &Location,
        dest_dir: &Location,
        chunk_size: u64,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Only support local files for now
        let (source_path, dest_path) = match (source, dest_dir) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => return OpResult::Failed("Split only supports local files".to_string()),
        };
        
        // Open source file
        let mut file = match tokio::fs::File::open(source_path).await {
            Ok(f) => f,
            Err(e) => return OpResult::Failed(format!("Failed to open source file: {}", e)),
        };
        
        // Get file size
        let metadata = match file.metadata().await {
            Ok(m) => m,
            Err(e) => return OpResult::Failed(format!("Failed to get file metadata: {}", e)),
        };
        let total_size = metadata.len();
        
        // Calculate number of chunks (for potential future use)
        let _num_chunks = total_size.div_ceil(chunk_size);
        
        // Get base filename
        let base_name = source_path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("file");
        
        // Split the file
        use tokio::io::AsyncReadExt;
        let mut buffer = vec![0u8; chunk_size as usize];
        let mut chunk_index = 0;
        let mut bytes_read_total = 0u64;
        
        loop {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }
            
            // Read chunk
            let bytes_read = match file.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => n,
                Err(e) => return OpResult::Failed(format!("Failed to read from source: {}", e)),
            };
            
            // Write chunk to file
            let chunk_name = format!("{}.part{:03}", base_name, chunk_index);
            let chunk_path = dest_path.join(&chunk_name);
            
            match tokio::fs::write(&chunk_path, &buffer[..bytes_read]).await {
                Ok(_) => {},
                Err(e) => return OpResult::Failed(format!("Failed to write chunk {}: {}", chunk_index, e)),
            }
            
            chunk_index += 1;
            bytes_read_total += bytes_read as u64;
            
            // Report progress
            let progress = bytes_read_total as f64 / total_size as f64;
                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
        }
        
        OpResult::Success(SuccessData::None)
    }
    
    async fn execute_join_files(
        &self,
        parts: &[Location],
        dest: &Location,
        spec: &JobSpec,
    ) -> OpResult {
        // Check for cancellation
        if spec.cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        // Only support local files for now
        let dest_path = match dest {
            Location::Local(path) => path,
            _ => return OpResult::Failed("Join only supports local files".to_string()),
        };
        
        // Create destination file
        let mut dest_file = match tokio::fs::File::create(dest_path).await {
            Ok(f) => f,
            Err(e) => return OpResult::Failed(format!("Failed to create destination file: {}", e)),
        };
        
        // Join all parts
        use tokio::io::AsyncWriteExt;
        let total_parts = parts.len();
        
        for (index, part) in parts.iter().enumerate() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                return OpResult::Cancelled;
            }
            
            let part_path = match part {
                Location::Local(path) => path,
                _ => return OpResult::Failed("All parts must be local files".to_string()),
            };
            
            // Read part
            let contents = match tokio::fs::read(part_path).await {
                Ok(data) => data,
                Err(e) => return OpResult::Failed(format!("Failed to read part {}: {}", index, e)),
            };
            
            // Write to destination
            match dest_file.write_all(&contents).await {
                Ok(_) => {},
                Err(e) => return OpResult::Failed(format!("Failed to write part {}: {}", index, e)),
            }
            
            // Report progress
            let progress = (index + 1) as f64 / total_parts as f64;
                    if let Err(e) = self.event_sender.send(JobEvent::Progress(spec.id, progress)) {
                        tracing::error!("Failed to send job progress event for {:?}: {}", spec.id, e);
                    }
        }
        
        OpResult::Success(SuccessData::None)
    }
    
    // Helper methods for file comparison
    
    async fn read_file_as_string(&self, location: &Location) -> Result<String, String> {
        match location {
            Location::Local(path) => {
                tokio::fs::read_to_string(path)
                    .await
                    .map_err(|e| e.to_string())
            }
            _ => Err("Only local files supported for comparison".to_string()),
        }
    }
    
    fn compute_diff(&self, left_lines: &[String], right_lines: &[String]) -> Vec<crate::job::DiffChunk> {
        use crate::job::{DiffChunk, DiffType};
        
        let mut chunks = Vec::new();
        let mut left_idx = 0;
        let mut right_idx = 0;
        
        while left_idx < left_lines.len() || right_idx < right_lines.len() {
            // Find matching lines
            let mut equal_lines = Vec::new();
            while left_idx < left_lines.len() 
                && right_idx < right_lines.len() 
                && left_lines[left_idx] == right_lines[right_idx] 
            {
                equal_lines.push(left_lines[left_idx].clone());
                left_idx += 1;
                right_idx += 1;
            }
            
            if !equal_lines.is_empty() {
                chunks.push(DiffChunk {
                    left_start: left_idx - equal_lines.len(),
                    left_lines: equal_lines.clone(),
                    right_start: right_idx - equal_lines.len(),
                    right_lines: equal_lines,
                    chunk_type: DiffType::Equal,
                });
            }
            
            // Find differences
            if left_idx < left_lines.len() && right_idx < right_lines.len() {
                // Both have lines - this is a modification
                let left_diff = vec![left_lines[left_idx].clone()];
                let right_diff = vec![right_lines[right_idx].clone()];
                left_idx += 1;
                right_idx += 1;
                
                chunks.push(DiffChunk {
                    left_start: left_idx - 1,
                    left_lines: left_diff,
                    right_start: right_idx - 1,
                    right_lines: right_diff,
                    chunk_type: DiffType::Modified,
                });
            } else if left_idx < left_lines.len() {
                // Only left has lines - deletion
                let deleted = vec![left_lines[left_idx].clone()];
                left_idx += 1;
                
                chunks.push(DiffChunk {
                    left_start: left_idx - 1,
                    left_lines: deleted,
                    right_start: right_idx,
                    right_lines: Vec::new(),
                    chunk_type: DiffType::Deleted,
                });
            } else if right_idx < right_lines.len() {
                // Only right has lines - addition
                let added = vec![right_lines[right_idx].clone()];
                right_idx += 1;
                
                chunks.push(DiffChunk {
                    left_start: left_idx,
                    left_lines: Vec::new(),
                    right_start: right_idx - 1,
                    right_lines: added,
                    chunk_type: DiffType::Added,
                });
            }
        }
        
        chunks
    }
    
    // Helper methods
    
    /// Get the filename from a location
    fn get_filename(&self, location: &Location) -> Option<String> {
        match location {
            Location::Local(path) => {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|s| s.to_string())
            }
            Location::Ssh { path, .. } => {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|s| s.to_string())
            }
            Location::Cloud { path, .. } => {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|s| s.to_string())
            }
            Location::Archive { inner_path, .. } => {
                inner_path.file_name()
                    .and_then(|name| name.to_str())
                    .map(|s| s.to_string())
            }
        }
    }
    
    /// Join a filename to a location
    fn join_location(&self, location: &Location, filename: &str) -> Location {
        match location {
            Location::Local(path) => Location::Local(path.join(filename)),
            Location::Ssh { host, port, path } => Location::Ssh {
                host: host.clone(),
                port: *port,
                path: path.join(filename),
            },
            Location::Cloud { provider, bucket, path } => Location::Cloud {
                provider: provider.clone(),
                bucket: bucket.clone(),
                path: path.join(filename),
            },
            Location::Archive { archive_path, inner_path } => Location::Archive {
                archive_path: archive_path.clone(),
                inner_path: inner_path.join(filename),
            },
        }
    }

    /// Get a display string for a location
    fn location_display(&self, location: &Location) -> String {
        match location {
            Location::Local(path) => path.display().to_string(),
            Location::Ssh { host, port, path } => {
                format!("ssh://{}:{}{}", host, port, path.display())
            }
            Location::Cloud { provider, bucket, path } => {
                format!("{}://{}/{}", provider, bucket, path.display())
            }
            Location::Archive { archive_path, inner_path } => {
                format!("{}#{}", self.location_display(archive_path), inner_path.display())
            }
        }
    }

    /// Execute a countdown test job
    /// 
    /// Counts down from start_value to 0, sleeping 1 second between each count.
    /// Sends progress updates every second.
    /// Supports cancellation via cancel_token.
    /// NOTE: JobEvent::Started is sent by execute() wrapper, not here
    async fn execute_countdown(
        &self,
        _duration_secs: u32,
        start_value: u32,
        spec: &JobSpec,
    ) -> OpResult {
        let job_id = spec.id;
        let total = start_value;

        tracing::debug!("CountDownJob: Starting job_id={:?} start_value={}", job_id, start_value);

        // Countdown loop
        for remaining in (0..=start_value).rev() {
            // Check for cancellation
            if spec.cancel_token.is_cancelled() {
                tracing::debug!("CountDownJob: Cancelled at remaining={} job_id={:?}", remaining, job_id);
                return OpResult::Cancelled;
            }
            
            // Calculate progress (0.0 to 1.0)
            let progress = if total > 0 {
                (total - remaining) as f64 / total as f64
            } else {
                1.0
            };
            
            // Send progress update with message
            let progress_msg = format!("Countdown: {}/{} seconds", remaining, total);
            let detail_msg = format!("Countdown test job - {} of {} seconds", remaining, total);
            
            let _ = self.event_sender.send(JobEvent::ProgressWithDetail(
                job_id,
                progress,
                progress_msg,
                detail_msg,
            ));
            
            // Sleep for 1 second (unless cancelled)
            tokio::select! {
                _ = tokio::time::sleep(tokio::time::Duration::from_secs(1)) => {
                    // Continue to next iteration
                }
                _ = async {
                    // Check cancellation during sleep
                    loop {
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        if spec.cancel_token.is_cancelled() {
                            break;
                        }
                    }
                } => {
                    return OpResult::Cancelled;
                }
            }
        }
        
        OpResult::Success(SuccessData::None)
    }

    /// Walk the filesystem collecting jump-navigation candidates.
    /// Yields periodically so other tasks can run; respects cancellation.
    async fn execute_collect_jump_candidates(
        &self,
        root: &str,
        include_files: bool,
        max_results: usize,
        max_depth: usize,
        cancel: &CancellationToken,
    ) -> OpResult {
        const IGNORE: &[&str] = &[
            ".git", ".svn", ".hg", "node_modules", "target", "__pycache__",
            ".cache", ".tox", "venv", ".venv",
        ];

        let mut candidates: Vec<String> = Vec::new();
        let mut stack: Vec<(std::path::PathBuf, usize)> = vec![(std::path::PathBuf::from(root), 0)];

        while let Some((dir, depth)) = stack.pop() {
            if cancel.is_cancelled() {
                return OpResult::Cancelled;
            }
            if candidates.len() >= max_results {
                break;
            }
            if let Ok(read_dir) = std::fs::read_dir(&dir) {
                for entry in read_dir.flatten() {
                    if cancel.is_cancelled() {
                        return OpResult::Cancelled;
                    }
                    let path = entry.path();
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if IGNORE.iter().any(|ign| name_str == *ign) {
                        continue;
                    }
                    let is_dir = path.is_dir();
                    if is_dir || include_files {
                        let p = path.to_string_lossy().into_owned();
                        candidates.push(p);
                        if candidates.len() >= max_results {
                            break;
                        }
                    }
                    if is_dir && depth < max_depth {
                        stack.push((path, depth + 1));
                    }
                }
            }
            // Yield after each directory to keep the event loop responsive
            tokio::task::yield_now().await;
        }

        OpResult::Success(SuccessData::JumpCandidates(candidates))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::LocalFilesystemBackend;
    use crate::job::{JobSpec, JobKind};
    use tempfile::TempDir;
    use crate::backend::MockArchiveHandler;

    #[tokio::test]
    async fn test_execute_read_directory() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        
        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(temp_dir.path().to_path_buf()),
        });
        
        executor.execute(spec).await;
        
        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        
        // Then a completed event
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Completed(_, SuccessData::DirectoryRead(_)))));
    }

    #[tokio::test]
    async fn test_execute_mkdir() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        
        let new_dir = temp_dir.path().join("test_dir");
        let spec = JobSpec::new(JobKind::Mkdir {
            location: Location::Local(new_dir.clone()),
        });
        
        executor.execute(spec).await;
        
        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));
        
        // Then a completed event
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Completed(_, SuccessData::None))));
        
        // Directory should exist
        assert!(new_dir.exists());
    }

    #[tokio::test]
    async fn test_execute_copy() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        
        // Create a source file
        let source_file = temp_dir.path().join("source.txt");
        tokio::fs::write(&source_file, b"test content").await.unwrap();
        
        let dest_dir = temp_dir.path().join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();
        
        let spec = JobSpec::new(JobKind::Copy {
            sources: vec![Location::Local(source_file.clone())],
            dest: Location::Local(dest_dir.clone()),
        });
        
        executor.execute(spec).await;
        
        // Should receive started, progress, and completed events
        let mut received_started = false;
        let mut received_completed = false;
        
        while let Ok(event) = event_rx.try_recv() {
            match event {
                JobEvent::Started(_) => received_started = true,
                JobEvent::Completed(_, _) => received_completed = true,
                _ => {}
            }
        }
        
        assert!(received_started);
        assert!(received_completed);
        
        // Destination file should exist
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
    }

    #[tokio::test]
    async fn test_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let backend = Arc::new(LocalFilesystemBackend::new());
        let archive_handler = Arc::new(MockArchiveHandler);
        let (event_tx, mut event_rx) = mpsc::unbounded_channel();
        
        let executor = JobExecutor::new(backend, archive_handler, event_tx);
        
        let spec = JobSpec::new(JobKind::ReadDirectory {
            location: Location::Local(temp_dir.path().to_path_buf()),
        });
        
        // Cancel the job before execution
        spec.cancel_token.cancel();
        
        executor.execute(spec).await;
        
        // Should receive a started event first
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Started(_))));

        // Then a cancelled event
        let event = event_rx.recv().await;
        assert!(matches!(event, Some(JobEvent::Cancelled(_))));
    }
}

// ============================================================================
// Conflict Detection Helper Function
// ============================================================================

/// Detect file conflicts between source files and destination
///
/// Returns a list of file conflicts (directories are logged and skipped)
pub async fn detect_conflicts(
    sources: &[Location],
    dest: &Location,
    backend: &dyn FilesystemBackend,
    job_id: crate::job::JobId,
    tab_id: usize,
) -> Result<Vec<crate::model::dialog::ConflictPair>, String> {
    use chrono::Local;

    debug!("detect_conflicts: Checking {} sources against dest {:?}", sources.len(), dest);
    let mut conflicts = Vec::new();

    for source in sources {
        // Calculate destination path
        let dest_location = calculate_destination_path(source, dest);
        debug!("detect_conflicts: Source {:?} -> Dest {:?}", source, dest_location);

        // Check if destination exists
        match backend.get_entry(&dest_location).await {
            Ok(dest_entry) => {
                debug!("detect_conflicts: Destination exists: {:?}", dest_location);
                // Source entry for comparison
                let source_entry = match backend.get_entry(source).await {
                    Ok(entry) => entry,
                    Err(e) => {
                        debug!("detect_conflicts: Failed to read source {:?}: {}", source, e);
                        continue; // Skip if can't read source
                    }
                };

                let conflict = crate::model::dialog::ConflictPair {
                    source: source_entry.clone(),
                    dest: dest_entry,
                    source_path: source.clone(),
                    dest_path: dest_location.clone(),
                    is_directory: source_entry.is_dir,
                };

                if conflict.is_directory {
                    // Log directory conflict to task pane and skip
                    let timestamp = Local::now().format("[%H:%M:%S]");
                    let _log_msg = format!(
                        "{} [Job {}] [Tab {}] Copy: \"{}\" conflicts. Skipping",
                        timestamp,
                        &job_id.0.to_string()[..8],
                        tab_id + 1,
                        dest_location.display_path()
                    );
                    // Note: This log needs to be passed back via StateUpdateResult
                    // For now, we just skip directories
                } else {
                    debug!("detect_conflicts: Adding file conflict for {:?}", dest_location);
                    conflicts.push(conflict);
                }
            }
            Err(e) => {
                debug!("detect_conflicts: Destination does not exist: {:?} ({})", dest_location, e);
                // No conflict - destination doesn't exist
            }
        }
    }

    debug!("detect_conflicts: Found {} conflicts", conflicts.len());
    Ok(conflicts)
}

/// Helper function to calculate destination path for a source file
fn calculate_destination_path(source: &Location, dest: &Location) -> Location {
    match (source, dest) {
        (Location::Local(src_path), Location::Local(dest_path)) => {
            if dest_path.is_dir() {
                // Destination is a directory, append source filename
                if let Some(filename) = src_path.file_name() {
                    Location::Local(dest_path.join(filename))
                } else {
                    dest.clone()
                }
            } else {
                // Destination is a file, use as-is
                dest.clone()
            }
        }
        _ => dest.clone(), // For non-local locations, use dest as-is
    }
}
