//! Filesystem backend abstraction
//!
//! This module defines the FilesystemBackend trait for abstracting
//! file I/O operations across different storage types.

use crate::model::{AttributeChange, FileEntry, LinkCreateKind, Location, TimestampChange};
use anyhow::Result;
use tokio_util::sync::CancellationToken;

pub mod archive;
pub mod local;

#[cfg(test)]
mod local_properties;

pub use archive::{
    ArchiveHandler, IsoArchiveHandler, LzhArchiveHandler, MultiFormatArchiveHandler,
    RarArchiveHandler, SevenZArchiveHandler, TarArchiveHandler, ZipArchiveHandler,
};
pub use local::LocalFilesystemBackend;

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

    /// Create an empty file
    async fn create_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()>;

    /// Change file attributes. Only fields set to `Some` in `attrs` are changed;
    /// returns the attribute values as they were immediately before the change.
    async fn set_attributes(
        &self,
        location: &Location,
        attrs: &AttributeChange,
        cancel_token: &CancellationToken,
    ) -> Result<AttributeChange>;

    /// Change file timestamps. Only fields set to `Some` in `times` are changed;
    /// returns the timestamp values as they were immediately before the change.
    async fn set_timestamps(
        &self,
        location: &Location,
        times: &TimestampChange,
        cancel_token: &CancellationToken,
    ) -> Result<TimestampChange>;

    /// Create a link at `link_path` pointing to `target`. Fails if
    /// `link_path` already exists.
    async fn create_link(
        &self,
        target: &Location,
        link_path: &Location,
        kind: LinkCreateKind,
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
    async fn calculate_directory_size_with_progress(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
        progress_callback: Box<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<u64>;

    /// Read file content as bytes
    async fn read_file_content(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<u8>>;

    /// Get a single file entry
    async fn get_entry(&self, location: &Location) -> Result<FileEntry>;
}
