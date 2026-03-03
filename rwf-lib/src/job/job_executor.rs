//! Job executor for processing JobSpecs
//!
//! This module implements the JobExecutor that dispatches jobs to the
//! appropriate backend methods and sends JobEvent updates.

use crate::backend::{FilesystemBackend, ArchiveHandler};
use crate::job::{JobSpec, JobKind, OpResult, SuccessData, PipeToAction};
use crate::model::Location;
use crate::worker_pool::JobEvent;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use std::sync::Arc;

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
        let _ = self.event_sender.send(JobEvent::Started(job_id));
        
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
            JobKind::CreateArchive { sources, dest } => {
                self.execute_create_archive(sources, dest, &spec).await
            }
            JobKind::ExecuteCustomFunction { command, working_dir, pipe_to_action, shell } => {
                self.execute_custom_function(command, working_dir, pipe_to_action, &spec, shell.as_deref()).await
            }
            JobKind::Search { location, pattern, recursive } => {
                self.execute_search(location, pattern, *recursive, &spec).await
            }
            JobKind::LoadFileForViewer { location } => {
                self.execute_load_file_for_viewer(location, &spec.cancel_token).await
            }
            JobKind::PatternRename { targets, pattern } => {
                self.execute_pattern_rename(targets, pattern, &spec).await
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
        };
        
        // Send completion event based on result
        let event = match result {
            OpResult::Success(data) => JobEvent::Completed(job_id, data),
            OpResult::Failed(error) => JobEvent::Failed(job_id, error),
            OpResult::Cancelled => JobEvent::Cancelled(job_id),
        };
        
        let _ = self.event_sender.send(event);
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
        
        for (i, source) in sources.iter().enumerate() {
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
            let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
            
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
        let _ = self.event_sender.send(JobEvent::Progress(spec.id, 1.0));
        
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
        
        for (i, source) in sources.iter().enumerate() {
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
            let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
            
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
        let _ = self.event_sender.send(JobEvent::Progress(spec.id, 1.0));
        
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
            let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
            
            // Delete the file
            if let Err(e) = self.backend.delete_file(target, &spec.cancel_token).await {
                return OpResult::Failed(format!("Failed to delete {}: {}", 
                    self.location_display(target), e));
            }
        }
        
        // Send final progress
        let _ = self.event_sender.send(JobEvent::Progress(spec.id, 1.0));
        
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
            move |items_processed, _current_size| {
                // Send progress updates every 100ms to avoid flooding
                let mut last = last_update.lock().unwrap();
                if last.elapsed() > std::time::Duration::from_millis(100) {
                    // We don't know the total, so we can't calculate a percentage
                    // Send a progress value that indicates activity (oscillating between 0.3 and 0.7)
                    let progress = 0.5 + 0.2 * ((items_processed % 10) as f64 / 10.0 - 0.5);
                    let _ = event_sender.send(JobEvent::Progress(job_id, progress));
                    *last = std::time::Instant::now();
                }
            }
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
        
        // Determine shell to use
        let (shell_cmd, shell_arg) = if let Some(shell_name) = shell {
            match shell_name {
                "bash" => ("bash", "-c"),
                "zsh" => ("zsh", "-c"),
                "powershell" => ("powershell", "-Command"),
                "cmd" => ("cmd", "/C"),
                _ => {
                    // Default based on OS
                    #[cfg(target_os = "windows")]
                    { ("cmd", "/C") }
                    #[cfg(not(target_os = "windows"))]
                    { ("sh", "-c") }
                }
            }
        } else {
            // Default based on OS
            #[cfg(target_os = "windows")]
            { ("cmd", "/C") }
            #[cfg(not(target_os = "windows"))]
            { ("sh", "-c") }
        };
        
        // Execute the command
        let output = tokio::process::Command::new(shell_cmd)
            .arg(shell_arg)
            .arg(command)
            .current_dir(working_path)
            .output()
            .await;
        
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
    
    /// Execute a load file for viewer operation
    async fn execute_load_file_for_viewer(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> OpResult {
        // Check for cancellation
        if cancel_token.is_cancelled() {
            return OpResult::Cancelled;
        }
        
        match location {
            Location::Local(path) => {
                match tokio::fs::read(path).await {
                    Ok(contents) => OpResult::Success(SuccessData::FileContents(contents)),
                    Err(e) => OpResult::Failed(format!("Failed to read file: {}", e)),
                }
            }
            Location::Archive { archive_path: _, inner_path: _ } => {
                // For archive files, we need to extract the specific file
                // This is a placeholder - actual implementation would use the archive backend
                OpResult::Failed("Archive file viewing not yet implemented".to_string())
            }
            _ => {
                OpResult::Failed("Unsupported location type for file viewing".to_string())
            }
        }
    }
    
    /// Execute a pattern rename operation
    async fn execute_pattern_rename(
        &self,
        targets: &[Location],
        pattern: &str,
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
            let new_name = match crate::pattern_rename::apply_pattern(&current_name, pattern) {
                Some(name) => name,
                None => {
                    // Pattern doesn't match this file, skip it
                    continue;
                }
            };
            
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
                    let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
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
        let _num_chunks = (total_size + chunk_size - 1) / chunk_size;
        
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
            let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
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
            let _ = self.event_sender.send(JobEvent::Progress(spec.id, progress));
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
