//! Filesystem backend abstraction
//!
//! This module defines the FilesystemBackend trait for abstracting
//! file I/O operations across different storage types.

use crate::model::{AttributeChange, FileEntry, LinkCreateKind, Location, TimestampChange};
use anyhow::Result;
use tokio_util::sync::CancellationToken;

pub mod archive;
pub mod local;
pub mod trash;

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

    /// Move a file/dir to the OS trash (or `.rwf-trash` fallback), returning
    /// a record with enough detail to restore it later. `force_fallback`
    /// skips the OS trash call entirely (see `TrashConfig.force_fallback`).
    async fn move_to_trash(
        &self,
        location: &Location,
        force_fallback: bool,
        cancel_token: &CancellationToken,
    ) -> Result<crate::model::TrashRecord>;

    /// Restore a previously trashed item back to its original location.
    async fn restore_from_trash(
        &self,
        record: &crate::model::TrashRecord,
        cancel_token: &CancellationToken,
    ) -> Result<()>;

    /// Permanently purge trash contents. `scope` selects OS-managed trash,
    /// the `.rwf-trash` fallback dirs under `fallback_roots`, or both. See
    /// `backend::trash::{purge_os_trash_sync, purge_fallback_dirs_sync}`
    /// for exact semantics. Returns the number of items purged.
    async fn empty_trash(
        &self,
        scope: crate::model::EmptyTrashScope,
        older_than_days: Option<u32>,
        fallback_roots: &[std::path::PathBuf],
    ) -> Result<usize>;

    /// Non-destructively count items and sum byte sizes across OS-managed
    /// trash and every `.rwf-trash` fallback dir under `fallback_roots`.
    /// Returns `(count, total_size)`. See `backend::trash::scan_os_trash_sync`
    /// for the one known limitation (directory byte sizes inside OS-managed
    /// trash are undercounted — the OS trash API doesn't expose them).
    async fn scan_trash(
        &self,
        fallback_roots: &[std::path::PathBuf],
        cancel_token: &CancellationToken,
    ) -> Result<(usize, u64)>;

    /// Non-destructively list every trashed item (OS-managed + `.rwf-trash`
    /// fallback dirs under `fallback_roots`), newest-deleted-first, for the
    /// trash-browser UI (Phase 7.7 Task 16). See `backend::trash::list_trash_sync`.
    async fn list_trash(
        &self,
        fallback_roots: &[std::path::PathBuf],
    ) -> Result<Vec<crate::model::TrashRecord>>;

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
