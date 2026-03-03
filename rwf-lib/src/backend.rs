//! Filesystem backend abstraction
//!
//! This module defines the FilesystemBackend trait for abstracting
//! file I/O operations across different storage types.

use crate::model::{Location, FileEntry};
use tokio_util::sync::CancellationToken;
use anyhow::Result;

pub mod local;
pub mod archive;

#[cfg(test)]
mod local_properties;

pub use local::LocalFilesystemBackend;
pub use archive::{ArchiveHandler, ZipArchiveHandler};

#[cfg(test)]
pub use archive::MockArchiveHandler;

/// Trait for filesystem operations
#[async_trait::async_trait]
pub trait FilesystemBackend: Send + Sync {
    /// Read directory contents
    async fn read_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>>;
    
    /// Copy a file
    async fn copy_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Move a file
    async fn move_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Delete a file
    async fn delete_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Rename a file
    async fn rename_file(
        &self,
        from: &Location,
        to: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Create a directory
    async fn create_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;
    
    /// Calculate directory size recursively
    async fn calculate_directory_size(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<u64>;
    
    /// Calculate directory size recursively with progress callback
    /// 
    /// The progress callback receives (items_processed, current_size) periodically
    async fn calculate_directory_size_with_progress<F>(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
        progress_callback: F,
    ) -> Result<u64>
    where
        F: Fn(u64, u64) + Send + Sync;
    
    /// Read file content as bytes
    async fn read_file_content(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<u8>>;
}
