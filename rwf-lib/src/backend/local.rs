//! Local filesystem backend implementation
//!
//! This module implements the FilesystemBackend trait for local filesystem operations.

use crate::backend::FilesystemBackend;
use crate::model::{AttributeChange, FileEntry, LinkCreateKind, Location, TimestampChange};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::time::SystemTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

/// Local filesystem backend
pub struct LocalFilesystemBackend {
    buffer_size: usize,
}

impl LocalFilesystemBackend {
    /// Create a new local filesystem backend with default buffer size
    pub fn new() -> Self {
        Self {
            buffer_size: 8192, // 8KB default buffer
        }
    }

    /// Create a new local filesystem backend with custom buffer size
    pub fn with_buffer_size(buffer_size: usize) -> Self {
        Self { buffer_size }
    }
}

impl Default for LocalFilesystemBackend {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FilesystemBackend for LocalFilesystemBackend {
    async fn read_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        use tracing::{debug, warn};

        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        debug!("Reading directory: {}", location.display_path());

        let mut entries = Vec::new();
        let mut read_dir = tokio::fs::read_dir(path)
            .await
            .with_context(|| format!("Failed to read directory {}", path.display()))?;

        while let Some(entry) = read_dir.next_entry().await? {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }

            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // symlink_metadata() returns the entry's OWN metadata without following
            // reparse points — critical on Windows where DirEntry::metadata() also
            // doesn't follow, but symlink_metadata() lets us call is_symlink() reliably.
            let sym_meta = tokio::fs::symlink_metadata(&entry_path).await?;

            #[cfg(target_os = "windows")]
            let is_hidden = {
                use std::os::windows::fs::MetadataExt;
                const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
                (sym_meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
            };
            #[cfg(not(target_os = "windows"))]
            let is_hidden = name.starts_with('.');

            let (is_dir, size, modified, is_symlink, link_target, link_kind) =
                if sym_meta.file_type().is_symlink() {
                    // Follow the link to get the target's real metadata for display.
                    // If the target is missing (broken link), fall back to symlink's own values.
                    let followed = tokio::fs::metadata(&entry_path).await.ok();
                    let is_dir = followed.as_ref().is_some_and(|m| m.is_dir());
                    let size = followed.as_ref().map_or(sym_meta.len(), |m| m.len());
                    let modified = followed
                        .as_ref()
                        .and_then(|m| m.modified().ok())
                        .or_else(|| sym_meta.modified().ok())
                        .unwrap_or_else(SystemTime::now);

                    let raw_target = tokio::fs::read_link(&entry_path).await.ok();
                    let link_kind = Some(
                        raw_target
                            .as_deref()
                            .map(crate::model::LinkKind::from_link_target)
                            .unwrap_or(crate::model::LinkKind::Symlink),
                    );

                    (is_dir, size, modified, true, raw_target, link_kind)
                } else {
                    let is_dir = sym_meta.is_dir();
                    let size = sym_meta.len();
                    let modified = sym_meta.modified().unwrap_or_else(|_| SystemTime::now());
                    (is_dir, size, modified, false, None, None)
                };

            let file_entry = FileEntry {
                name,
                location: Location::Local(entry_path),
                size,
                is_dir,
                is_hidden,
                modified,
                marked: false,
                calculated_size: None,
                is_symlink,
                link_target,
                link_kind,
            };

            entries.push(file_entry);
        }

        debug!(
            "Found {} entries in {}",
            entries.len(),
            location.display_path()
        );

        // Special warning for C:\ with 0 entries
        if location.display_path() == "C:\\" && entries.is_empty() {
            warn!("C:\\ returned 0 entries - this may indicate a permissions issue");
        }

        Ok(entries)
    }

    async fn copy_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (src_path, dest_path) = match (source, dest) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        // Determine destination path
        let final_dest = if dest_path.is_dir() {
            dest_path.join(src_path.file_name().context("Invalid source path")?)
        } else {
            dest_path.clone()
        };

        // Check if source is a directory
        let metadata = tokio::fs::metadata(src_path)
            .await
            .context("Failed to read source metadata")?;

        if metadata.is_dir() {
            self.copy_directory(src_path, &final_dest, cancel_token)
                .await?;
        } else {
            self.copy_single_file(src_path, &final_dest, cancel_token)
                .await?;
        }

        Ok(())
    }

    async fn move_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (src_path, dest_path) = match (source, dest) {
            (Location::Local(src), Location::Local(dst)) => (src, dst),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        // Determine destination path
        let final_dest = if dest_path.is_dir() {
            dest_path.join(src_path.file_name().context("Invalid source path")?)
        } else {
            dest_path.clone()
        };

        // Try atomic rename first (works if on same filesystem)
        match tokio::fs::rename(src_path, &final_dest).await {
            Ok(_) => Ok(()),
            Err(_) => {
                // If rename fails (cross-filesystem), copy then delete
                self.copy_file(source, dest, cancel_token).await?;
                self.delete_file(source, cancel_token).await?;
                Ok(())
            }
        }
    }

    async fn delete_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        let metadata = tokio::fs::metadata(path)
            .await
            .context("Failed to read file metadata")?;

        if metadata.is_dir() {
            tokio::fs::remove_dir_all(path)
                .await
                .context("Failed to delete directory")?;
        } else {
            tokio::fs::remove_file(path)
                .await
                .context("Failed to delete file")?;
        }

        Ok(())
    }

    async fn rename_file(
        &self,
        from: &Location,
        to: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (from_path, to_path) = match (from, to) {
            (Location::Local(from), Location::Local(to)) => (from, to),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        // Check if destination already exists
        if to_path.exists() {
            bail!("Destination file already exists");
        }

        // Validate filename (check for invalid characters)
        if let Some(filename) = to_path.file_name() {
            let filename_str = filename.to_string_lossy();
            if filename_str.contains(['<', '>', ':', '"', '|', '?', '*']) {
                bail!("Invalid characters in filename");
            }
        }

        tokio::fs::rename(from_path, to_path)
            .await
            .context("Failed to rename file")?;

        Ok(())
    }

    async fn create_directory(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        // Check if directory already exists
        if path.exists() {
            bail!("Directory already exists");
        }

        // Validate directory name
        if let Some(dirname) = path.file_name() {
            let dirname_str = dirname.to_string_lossy();
            if dirname_str.contains(['<', '>', ':', '"', '|', '?', '*']) {
                bail!("Invalid characters in directory name");
            }
        }

        tokio::fs::create_dir_all(path)
            .await
            .context("Failed to create directory")?;

        Ok(())
    }

    async fn create_file(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        // Check if file already exists
        if path.exists() {
            bail!("File already exists");
        }

        // Validate file name
        if let Some(filename) = path.file_name() {
            let filename_str = filename.to_string_lossy();
            if filename_str.contains(['<', '>', ':', '"', '|', '?', '*']) {
                bail!("Invalid characters in file name");
            }
        }

        tokio::fs::File::create(path)
            .await
            .context("Failed to create file")?;

        Ok(())
    }

    async fn set_attributes(
        &self,
        location: &Location,
        attrs: &AttributeChange,
        cancel_token: &CancellationToken,
    ) -> Result<AttributeChange> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::MetadataExt;
            const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
            const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
            const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
            const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;

            let old_bits = tokio::fs::metadata(path)
                .await
                .context("Failed to read file attributes")?
                .file_attributes();

            let old = AttributeChange {
                readonly: Some(old_bits & FILE_ATTRIBUTE_READONLY != 0),
                hidden: Some(old_bits & FILE_ATTRIBUTE_HIDDEN != 0),
                system: Some(old_bits & FILE_ATTRIBUTE_SYSTEM != 0),
                archive: Some(old_bits & FILE_ATTRIBUTE_ARCHIVE != 0),
            };

            let mut new_bits = old_bits;
            let mut apply_bit = |flag: u32, value: Option<bool>| {
                if let Some(set) = value {
                    if set {
                        new_bits |= flag;
                    } else {
                        new_bits &= !flag;
                    }
                }
            };
            apply_bit(FILE_ATTRIBUTE_READONLY, attrs.readonly);
            apply_bit(FILE_ATTRIBUTE_HIDDEN, attrs.hidden);
            apply_bit(FILE_ATTRIBUTE_SYSTEM, attrs.system);
            apply_bit(FILE_ATTRIBUTE_ARCHIVE, attrs.archive);

            if new_bits != old_bits {
                let path = path.clone();
                tokio::task::spawn_blocking(move || {
                    crate::volume_info::set_windows_file_attributes(&path, new_bits)
                })
                .await
                .context("set_attributes task panicked")??;
            }

            Ok(old)
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let metadata = tokio::fs::metadata(path)
                .await
                .context("Failed to read file permissions")?;
            let old_mode = metadata.permissions().mode() & 0o7777;
            let old = AttributeChange {
                mode: Some(old_mode),
            };

            if let Some(new_mode) = attrs.mode {
                if new_mode != old_mode {
                    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(new_mode))
                        .await
                        .context("Failed to set file permissions")?;
                }
            }

            Ok(old)
        }
    }

    async fn set_timestamps(
        &self,
        location: &Location,
        times: &TimestampChange,
        cancel_token: &CancellationToken,
    ) -> Result<TimestampChange> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        let metadata = tokio::fs::metadata(path)
            .await
            .context("Failed to read file timestamps")?;
        #[cfg(windows)]
        let old = TimestampChange {
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
            created: metadata.created().ok(),
        };
        #[cfg(not(windows))]
        let old = TimestampChange {
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
        };

        if times.modified.is_some() || times.accessed.is_some() {
            let new_modified = times
                .modified
                .or(old.modified)
                .unwrap_or(std::time::SystemTime::now());
            let new_accessed = times
                .accessed
                .or(old.accessed)
                .unwrap_or(std::time::SystemTime::now());
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                filetime::set_file_times(
                    &path,
                    filetime::FileTime::from_system_time(new_accessed),
                    filetime::FileTime::from_system_time(new_modified),
                )
            })
            .await
            .context("set_timestamps task panicked")?
            .context("Failed to set file timestamps")?;
        }

        #[cfg(windows)]
        if let Some(new_created) = times.created {
            let path = path.clone();
            tokio::task::spawn_blocking(move || {
                crate::volume_info::set_windows_creation_time(&path, new_created)
            })
            .await
            .context("set_creation_time task panicked")??;
        }

        Ok(old)
    }

    async fn create_link(
        &self,
        target: &Location,
        link_path: &Location,
        kind: LinkCreateKind,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let (target_path, link_path) = match (target, link_path) {
            (Location::Local(t), Location::Local(l)) => (t, l),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        if link_path.exists() {
            bail!("Link path already exists");
        }

        match kind {
            LinkCreateKind::Symlink => {
                #[cfg(windows)]
                {
                    let is_dir = target_path.is_dir();
                    let target_path = target_path.clone();
                    let link_path = link_path.clone();
                    tokio::task::spawn_blocking(move || {
                        if is_dir {
                            std::os::windows::fs::symlink_dir(&target_path, &link_path)
                        } else {
                            std::os::windows::fs::symlink_file(&target_path, &link_path)
                        }
                    })
                    .await
                    .context("create_link task panicked")?
                    .context("Failed to create symlink")?;
                }
                #[cfg(unix)]
                {
                    tokio::fs::symlink(target_path, link_path)
                        .await
                        .context("Failed to create symlink")?;
                }
            }
            LinkCreateKind::Hardlink => {
                tokio::fs::hard_link(target_path, link_path)
                    .await
                    .context("Failed to create hard link")?;
            }
            #[cfg(windows)]
            LinkCreateKind::Junction => {
                // `/D` disables cmd.exe's AutoRun (HKCU/HKLM
                // Software\Microsoft\Command Processor\AutoRun) — without it,
                // a registered shell enhancement (e.g. Clink) that fails to
                // inject itself pollutes the process exit code even though
                // mklink itself succeeded. Stdio is also fully detached, not
                // just piped, matching `execute_spawn_process`'s fix for the
                // same class of interference.
                let output = tokio::process::Command::new("cmd")
                    .args(["/D", "/C", "mklink", "/J"])
                    .arg(link_path)
                    .arg(target_path)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
                    .await
                    .context("Failed to spawn mklink")?;
                if !output.status.success() {
                    bail!(
                        "mklink /J failed (exit {:?}): stdout={:?} stderr={:?}",
                        output.status.code(),
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr)
                    );
                }
            }
        }

        Ok(())
    }

    async fn move_to_trash(
        &self,
        location: &Location,
        force_fallback: bool,
        cancel_token: &CancellationToken,
    ) -> Result<crate::model::TrashRecord> {
        let path = match location {
            Location::Local(path) => path.clone(),
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        tokio::task::spawn_blocking(move || {
            crate::backend::trash::move_to_trash_sync(&path, force_fallback)
        })
        .await
        .context("move_to_trash task panicked")?
    }

    async fn restore_from_trash(
        &self,
        record: &crate::model::TrashRecord,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }
        let record = record.clone();
        tokio::task::spawn_blocking(move || crate::backend::trash::restore_from_trash_sync(&record))
            .await
            .context("restore_from_trash task panicked")?
    }

    async fn empty_trash(
        &self,
        scope: crate::model::EmptyTrashScope,
        older_than_days: Option<u32>,
        fallback_roots: &[std::path::PathBuf],
    ) -> Result<usize> {
        use crate::model::EmptyTrashScope::{All, Fallback, OsManaged};
        let fallback_roots = fallback_roots.to_vec();
        tokio::task::spawn_blocking(move || {
            let mut purged = 0usize;
            if matches!(scope, OsManaged | All) {
                purged += crate::backend::trash::purge_os_trash_sync(older_than_days)?;
            }
            if matches!(scope, Fallback | All) {
                purged += crate::backend::trash::purge_fallback_dirs_sync(&fallback_roots)?;
            }
            Ok(purged)
        })
        .await
        .context("empty_trash task panicked")?
    }

    async fn scan_trash(
        &self,
        fallback_roots: &[std::path::PathBuf],
        cancel_token: &CancellationToken,
    ) -> Result<(usize, u64)> {
        let (os_count, os_size) =
            tokio::task::spawn_blocking(crate::backend::trash::scan_os_trash_sync)
                .await
                .context("scan_trash task panicked")??;

        let mut fallback_count = 0usize;
        let mut fallback_size = 0u64;
        for root in fallback_roots {
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }
            let trash_dir = root.join(".rwf-trash");
            if !trash_dir.exists() {
                continue;
            }
            let mut read_dir = tokio::fs::read_dir(&trash_dir)
                .await
                .context("failed to read .rwf-trash directory")?;
            while let Some(entry) = read_dir.next_entry().await? {
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }
                let path = entry.path();
                if path.to_string_lossy().ends_with(".rwf-meta.json") {
                    continue;
                }
                let metadata = entry.metadata().await?;
                fallback_size += if metadata.is_dir() {
                    self.calculate_dir_size_recursive(&path, cancel_token)
                        .await
                        .unwrap_or(0)
                } else {
                    metadata.len()
                };
                fallback_count += 1;
            }
        }

        Ok((os_count + fallback_count, os_size + fallback_size))
    }

    async fn list_trash(
        &self,
        fallback_roots: &[std::path::PathBuf],
    ) -> Result<Vec<crate::model::TrashRecord>> {
        let fallback_roots = fallback_roots.to_vec();
        tokio::task::spawn_blocking(move || crate::backend::trash::list_trash_sync(&fallback_roots))
            .await
            .context("list_trash task panicked")?
    }

    async fn calculate_directory_size(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<u64> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        self.calculate_dir_size_recursive(path, cancel_token).await
    }

    async fn calculate_directory_size_with_progress(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
        progress_callback: Box<dyn Fn(u64, u64) + Send + Sync>,
    ) -> Result<u64> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        let items_processed = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
        let current_size = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

        self.calculate_dir_size_recursive_with_progress(
            path,
            cancel_token,
            items_processed.clone(),
            current_size.clone(),
            &*progress_callback,
        )
        .await
    }

    async fn read_file_content(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<u8>> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        // Check cancellation before starting
        if cancel_token.is_cancelled() {
            bail!("Operation cancelled");
        }

        let mut file = tokio::fs::File::open(path)
            .await
            .context("Failed to open file")?;

        let mut contents = Vec::new();
        let mut buffer = vec![0u8; self.buffer_size];

        loop {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }

            let n = file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            contents.extend_from_slice(&buffer[..n]);
        }

        Ok(contents)
    }

    async fn get_entry(&self, location: &Location) -> Result<FileEntry> {
        let path = match location {
            Location::Local(path) => path,
            _ => bail!("LocalFilesystemBackend only supports Local locations"),
        };

        let metadata = tokio::fs::metadata(path)
            .await
            .context("Failed to read metadata")?;

        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Ok(FileEntry {
            name,
            location: location.clone(),
            size: metadata.len(),
            is_dir: metadata.is_dir(),
            is_hidden: is_hidden(path),
            modified: metadata.modified().unwrap_or_else(|_| SystemTime::now()),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        })
    }
}

// Private helper methods
impl LocalFilesystemBackend {
    /// Copy a single file with progress reporting
    async fn copy_single_file(
        &self,
        src: &Path,
        dest: &Path,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        let mut src_file = tokio::fs::File::open(src)
            .await
            .context("Failed to open source file")?;

        let mut dest_file = tokio::fs::File::create(dest)
            .await
            .context("Failed to create destination file")?;

        let mut buffer = vec![0u8; self.buffer_size];

        loop {
            // Check cancellation periodically
            if cancel_token.is_cancelled() {
                bail!("Operation cancelled");
            }

            let n = src_file.read(&mut buffer).await?;
            if n == 0 {
                break;
            }

            dest_file.write_all(&buffer[..n]).await?;
        }

        // Flush to ensure all data is written
        dest_file.flush().await?;

        Ok(())
    }

    /// Copy a directory recursively
    fn copy_directory<'a>(
        &'a self,
        src: &'a Path,
        dest: &'a Path,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + 'a>> {
        Box::pin(async move {
            // Create destination directory
            tokio::fs::create_dir_all(dest)
                .await
                .context("Failed to create destination directory")?;

            let mut read_dir = tokio::fs::read_dir(src)
                .await
                .context("Failed to read source directory")?;

            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }

                let src_path = entry.path();
                let dest_path = dest.join(entry.file_name());

                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    self.copy_directory(&src_path, &dest_path, cancel_token)
                        .await?;
                } else {
                    self.copy_single_file(&src_path, &dest_path, cancel_token)
                        .await?;
                }
            }

            Ok(())
        })
    }

    /// Calculate directory size recursively
    fn calculate_dir_size_recursive<'a>(
        &'a self,
        path: &'a Path,
        cancel_token: &'a CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total = 0u64;

            let mut read_dir = tokio::fs::read_dir(path)
                .await
                .with_context(|| format!("Failed to read directory {}", path.display()))?;

            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }

                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    total += self
                        .calculate_dir_size_recursive(&entry.path(), cancel_token)
                        .await?;
                } else {
                    total += metadata.len();
                }
            }

            Ok(total)
        })
    }

    /// Calculate directory size recursively with progress reporting
    fn calculate_dir_size_recursive_with_progress<'a>(
        &'a self,
        path: &'a Path,
        cancel_token: &'a CancellationToken,
        items_processed: std::sync::Arc<std::sync::atomic::AtomicU64>,
        current_size: std::sync::Arc<std::sync::atomic::AtomicU64>,
        progress_callback: &'a (dyn Fn(u64, u64) + Send + Sync),
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64>> + Send + 'a>> {
        Box::pin(async move {
            let mut total = 0u64;

            let mut read_dir = tokio::fs::read_dir(path)
                .await
                .with_context(|| format!("Failed to read directory {}", path.display()))?;

            while let Some(entry) = read_dir.next_entry().await? {
                // Check cancellation periodically
                if cancel_token.is_cancelled() {
                    bail!("Operation cancelled");
                }

                let metadata = entry.metadata().await?;

                if metadata.is_dir() {
                    total += self
                        .calculate_dir_size_recursive_with_progress(
                            &entry.path(),
                            cancel_token,
                            items_processed.clone(),
                            current_size.clone(),
                            progress_callback,
                        )
                        .await?;
                } else {
                    total += metadata.len();
                }

                // Update progress counters
                let items = items_processed.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                let size = current_size
                    .fetch_add(metadata.len(), std::sync::atomic::Ordering::Relaxed)
                    + metadata.len();

                // Report progress every 100 items
                if items.is_multiple_of(100) {
                    progress_callback(items, size);
                }
            }

            Ok(total)
        })
    }
}

/// Check if a file is hidden
#[cfg(target_os = "windows")]
fn is_hidden(path: &Path) -> bool {
    use std::os::windows::fs::MetadataExt;
    if let Ok(metadata) = path.metadata() {
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        (metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN) != 0
    } else {
        false
    }
}

/// Check if a file is hidden (Unix-like systems)
#[cfg(not(target_os = "windows"))]
fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.starts_with('.'))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create some test files
        tokio::fs::write(temp_path.join("file1.txt"), b"content1")
            .await
            .unwrap();
        tokio::fs::write(temp_path.join("file2.txt"), b"content2")
            .await
            .unwrap();
        tokio::fs::create_dir(temp_path.join("subdir"))
            .await
            .unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();

        let entries = backend
            .read_directory(&location, &cancel_token)
            .await
            .unwrap();

        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.name == "file1.txt"));
        assert!(entries.iter().any(|e| e.name == "file2.txt"));
        assert!(entries.iter().any(|e| e.name == "subdir" && e.is_dir));
    }

    #[tokio::test]
    async fn test_copy_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let src_path = temp_path.join("source.txt");
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        tokio::fs::write(&src_path, b"test content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let src_location = Location::Local(src_path.clone());
        let dest_location = Location::Local(dest_dir.clone());
        let cancel_token = CancellationToken::new();

        backend
            .copy_file(&src_location, &dest_location, &cancel_token)
            .await
            .unwrap();

        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content");
    }

    #[tokio::test]
    async fn test_move_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let src_path = temp_path.join("source.txt");
        let dest_dir = temp_path.join("dest");
        tokio::fs::create_dir(&dest_dir).await.unwrap();

        tokio::fs::write(&src_path, b"test content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let src_location = Location::Local(src_path.clone());
        let dest_location = Location::Local(dest_dir.clone());
        let cancel_token = CancellationToken::new();

        backend
            .move_file(&src_location, &dest_location, &cancel_token)
            .await
            .unwrap();

        assert!(!src_path.exists());
        let dest_file = dest_dir.join("source.txt");
        assert!(dest_file.exists());
        let content = tokio::fs::read(&dest_file).await.unwrap();
        assert_eq!(content, b"test content");
    }

    #[tokio::test]
    async fn test_delete_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let file_path = temp_path.join("to_delete.txt");
        tokio::fs::write(&file_path, b"delete me").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();

        backend.delete_file(&location, &cancel_token).await.unwrap();

        assert!(!file_path.exists());
    }

    #[tokio::test]
    async fn test_rename_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let old_path = temp_path.join("old_name.txt");
        let new_path = temp_path.join("new_name.txt");
        tokio::fs::write(&old_path, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let from_location = Location::Local(old_path.clone());
        let to_location = Location::Local(new_path.clone());
        let cancel_token = CancellationToken::new();

        backend
            .rename_file(&from_location, &to_location, &cancel_token)
            .await
            .unwrap();

        assert!(!old_path.exists());
        assert!(new_path.exists());
    }

    #[tokio::test]
    async fn test_create_directory() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let new_dir = temp_path.join("new_directory");

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(new_dir.clone());
        let cancel_token = CancellationToken::new();

        backend
            .create_directory(&location, &cancel_token)
            .await
            .unwrap();

        assert!(new_dir.exists());
        assert!(new_dir.is_dir());
    }

    #[tokio::test]
    async fn test_create_file() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let new_file = temp_path.join("new_file.txt");

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(new_file.clone());
        let cancel_token = CancellationToken::new();

        backend.create_file(&location, &cancel_token).await.unwrap();

        assert!(new_file.exists());
        assert!(new_file.is_file());
        assert_eq!(tokio::fs::metadata(&new_file).await.unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_create_file_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let existing_file = temp_path.join("existing.txt");
        tokio::fs::write(&existing_file, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(existing_file.clone());
        let cancel_token = CancellationToken::new();

        let result = backend.create_file(&location, &cancel_token).await;

        assert!(result.is_err());
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_set_attributes_windows_hidden() {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("visible.txt");
        tokio::fs::write(&file_path, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();

        let change = AttributeChange {
            readonly: None,
            hidden: Some(true),
            system: None,
            archive: None,
        };

        let old = backend
            .set_attributes(&location, &change, &cancel_token)
            .await
            .unwrap();

        // Newly-created file was not hidden before the change
        assert_eq!(old.hidden, Some(false));

        let attrs_after = tokio::fs::metadata(&file_path)
            .await
            .unwrap()
            .file_attributes();
        assert_ne!(attrs_after & FILE_ATTRIBUTE_HIDDEN, 0);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_set_attributes_unix_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("script.sh");
        tokio::fs::write(&file_path, b"content").await.unwrap();
        tokio::fs::set_permissions(&file_path, std::fs::Permissions::from_mode(0o644))
            .await
            .unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();

        let change = AttributeChange { mode: Some(0o755) };

        let old = backend
            .set_attributes(&location, &change, &cancel_token)
            .await
            .unwrap();

        assert_eq!(old.mode, Some(0o644));

        let mode_after = tokio::fs::metadata(&file_path)
            .await
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode_after, 0o755);
    }

    #[tokio::test]
    async fn test_set_timestamps_modified() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("stamped.txt");
        tokio::fs::write(&file_path, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();

        let new_modified =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        let change = TimestampChange {
            modified: Some(new_modified),
            accessed: None,
            #[cfg(windows)]
            created: None,
        };

        let old = backend
            .set_timestamps(&location, &change, &cancel_token)
            .await
            .unwrap();

        assert!(old.modified.is_some());
        assert_ne!(old.modified, Some(new_modified));

        let modified_after = tokio::fs::metadata(&file_path)
            .await
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(modified_after, new_modified);
    }

    #[cfg(target_os = "windows")]
    #[tokio::test]
    async fn test_set_timestamps_created_windows() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("stamped.txt");
        tokio::fs::write(&file_path, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(file_path.clone());
        let cancel_token = CancellationToken::new();

        let new_created =
            std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        let change = TimestampChange {
            modified: None,
            accessed: None,
            created: Some(new_created),
        };

        let old = backend
            .set_timestamps(&location, &change, &cancel_token)
            .await
            .unwrap();

        assert!(old.created.is_some());
        assert_ne!(old.created, Some(new_created));

        let created_after = tokio::fs::metadata(&file_path)
            .await
            .unwrap()
            .created()
            .unwrap();
        assert_eq!(created_after, new_created);
    }

    #[tokio::test]
    async fn test_create_link_hardlink() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        tokio::fs::write(&target, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();

        backend
            .create_link(
                &Location::Local(target.clone()),
                &Location::Local(link.clone()),
                LinkCreateKind::Hardlink,
                &cancel_token,
            )
            .await
            .unwrap();

        assert!(link.exists());
        assert_eq!(tokio::fs::read(&link).await.unwrap(), b"content");
    }

    #[tokio::test]
    async fn test_create_link_symlink_file() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        tokio::fs::write(&target, b"content").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();

        backend
            .create_link(
                &Location::Local(target.clone()),
                &Location::Local(link.clone()),
                LinkCreateKind::Symlink,
                &cancel_token,
            )
            .await
            .unwrap();

        let sym_meta = tokio::fs::symlink_metadata(&link).await.unwrap();
        assert!(sym_meta.file_type().is_symlink());
    }

    #[tokio::test]
    async fn test_create_link_already_exists() {
        let temp_dir = TempDir::new().unwrap();
        let target = temp_dir.path().join("target.txt");
        let link = temp_dir.path().join("link.txt");
        tokio::fs::write(&target, b"content").await.unwrap();
        tokio::fs::write(&link, b"existing").await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();

        let result = backend
            .create_link(
                &Location::Local(target),
                &Location::Local(link),
                LinkCreateKind::Hardlink,
                &cancel_token,
            )
            .await;

        assert!(result.is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn test_create_link_junction_dir() {
        let temp_dir = TempDir::new().unwrap();
        let target_dir = temp_dir.path().join("target_dir");
        let link = temp_dir.path().join("link_dir");
        tokio::fs::create_dir(&target_dir).await.unwrap();
        tokio::fs::write(target_dir.join("inside.txt"), b"x")
            .await
            .unwrap();

        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();

        backend
            .create_link(
                &Location::Local(target_dir),
                &Location::Local(link.clone()),
                LinkCreateKind::Junction,
                &cancel_token,
            )
            .await
            .unwrap();

        assert!(link.join("inside.txt").exists());
    }

    #[tokio::test]
    async fn test_calculate_directory_size() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create files with known sizes
        tokio::fs::write(temp_path.join("file1.txt"), vec![0u8; 100])
            .await
            .unwrap();
        tokio::fs::write(temp_path.join("file2.txt"), vec![0u8; 200])
            .await
            .unwrap();

        let subdir = temp_path.join("subdir");
        tokio::fs::create_dir(&subdir).await.unwrap();
        tokio::fs::write(subdir.join("file3.txt"), vec![0u8; 300])
            .await
            .unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();

        let size = backend
            .calculate_directory_size(&location, &cancel_token)
            .await
            .unwrap();

        assert_eq!(size, 600); // 100 + 200 + 300
    }

    #[tokio::test]
    async fn test_calculate_directory_size_with_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a directory structure with many files
        for i in 0..10 {
            let subdir = temp_path.join(format!("subdir{}", i));
            tokio::fs::create_dir(&subdir).await.unwrap();
            for j in 0..10 {
                tokio::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100])
                    .await
                    .unwrap();
            }
        }

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();

        // Cancel the token immediately
        cancel_token.cancel();

        // The operation should fail with cancellation error
        let result = backend
            .calculate_directory_size(&location, &cancel_token)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[tokio::test]
    async fn test_calculate_directory_size_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        // Create a directory structure with many files to trigger progress reporting
        for i in 0..5 {
            let subdir = temp_path.join(format!("subdir{}", i));
            tokio::fs::create_dir(&subdir).await.unwrap();
            for j in 0..25 {
                tokio::fs::write(subdir.join(format!("file{}.txt", j)), vec![0u8; 100])
                    .await
                    .unwrap();
            }
        }

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(temp_path.to_path_buf());
        let cancel_token = CancellationToken::new();

        // Track progress updates
        let progress_updates = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let progress_updates_clone = progress_updates.clone();

        let size = backend
            .calculate_directory_size_with_progress(
                &location,
                &cancel_token,
                Box::new(move |items, size| {
                    progress_updates_clone.lock().unwrap().push((items, size));
                }),
            )
            .await
            .unwrap();

        // Verify the total size is correct (5 subdirs * 25 files * 100 bytes = 12500)
        assert_eq!(size, 12500);

        // Verify we received progress updates
        let updates = progress_updates.lock().unwrap();
        assert!(!updates.is_empty(), "Should have received progress updates");

        // Verify progress updates are increasing
        for i in 1..updates.len() {
            assert!(
                updates[i].0 >= updates[i - 1].0,
                "Items should be increasing"
            );
            assert!(
                updates[i].1 >= updates[i - 1].1,
                "Size should be increasing"
            );
        }
    }

    #[tokio::test]
    async fn test_cancellation() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();

        let src_path = temp_path.join("source.txt");
        tokio::fs::write(&src_path, vec![0u8; 1000]).await.unwrap();

        let backend = LocalFilesystemBackend::new();
        let location = Location::Local(src_path);
        let cancel_token = CancellationToken::new();

        // Cancel immediately
        cancel_token.cancel();

        let result = backend.read_file_content(&location, &cancel_token).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cancelled"));
    }

    #[tokio::test]
    async fn test_move_to_trash_and_restore_round_trip() {
        if crate::test_utils::is_ci() {
            eprintln!("skipping: CI's OS trash has no restore-tracking metadata");
            return;
        }
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("backend_trash_test.txt");
        std::fs::write(&file_path, b"hello").unwrap();
        let location = Location::Local(file_path.clone());
        let backend = LocalFilesystemBackend::new();
        let cancel_token = CancellationToken::new();

        let record = backend
            .move_to_trash(&location, false, &cancel_token)
            .await
            .expect("move_to_trash should succeed");
        assert!(!file_path.exists());

        backend
            .restore_from_trash(&record, &cancel_token)
            .await
            .expect("restore_from_trash should succeed");
        assert!(file_path.exists());
        assert_eq!(std::fs::read(&file_path).unwrap(), b"hello");
    }

    /// Best-effort cleanup of a real `.rwf-trash` directory this test wrote
    /// to at the true filesystem volume root (not a `TempDir`). Runs on
    /// `Drop` so it fires even if an assertion in the test body panics,
    /// letting the test's own assertions run in their natural order without
    /// a manual cleanup step short-circuiting what they're actually
    /// verifying.
    struct RealTrashDirCleanup(std::path::PathBuf);
    impl Drop for RealTrashDirCleanup {
        fn drop(&mut self) {
            if let Ok(entries) = std::fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_empty_trash_scoped_to_fallback_only() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("scoped_purge.txt");
        std::fs::write(&file_path, b"x").unwrap();
        let backend = LocalFilesystemBackend::new();

        // `force_fallback=true` moves into `.rwf-trash` anchored at the true
        // filesystem volume root (`backend::trash::volume_root`), not under
        // `dir.path()` — scoping `fallback_roots`/the assertion to the real
        // volume root mirrors production behavior instead of asserting
        // against a directory that was never written to. This does briefly
        // touch the real drive root, so `_cleanup` guarantees it's cleaned
        // up unconditionally, even if an assertion below panics.
        let volume_root = dir.path().ancestors().last().unwrap().to_path_buf();
        let trash_dir = volume_root.join(".rwf-trash");
        let _cleanup = RealTrashDirCleanup(trash_dir.clone());

        backend
            .move_to_trash(
                &Location::Local(file_path.clone()),
                true,
                &CancellationToken::new(),
            )
            .await
            .expect("forced fallback move should succeed");

        let purged = backend
            .empty_trash(
                crate::model::EmptyTrashScope::Fallback,
                None,
                std::slice::from_ref(&volume_root),
            )
            .await
            .expect("empty_trash should succeed");

        assert!(purged >= 1);
        assert!(
            std::fs::read_dir(&trash_dir)
                .map(|mut e| e.next().is_none())
                .unwrap_or(true),
            ".rwf-trash should be empty after purge"
        );
    }

    #[tokio::test]
    async fn test_scan_trash_sums_fallback_files_and_recursive_dir_sizes() {
        let dir = TempDir::new().unwrap();
        let backend = LocalFilesystemBackend::new();

        let volume_root = dir.path().ancestors().last().unwrap().to_path_buf();
        let trash_dir = volume_root.join(".rwf-trash");
        let _cleanup = RealTrashDirCleanup(trash_dir.clone());

        // Baseline before trashing anything — scan_trash also folds in the real
        // OS-managed trash, which may already hold unrelated items on a dev
        // machine, so assertions below are deltas, not absolute counts.
        let (before_count, before_size) = backend
            .scan_trash(
                std::slice::from_ref(&volume_root),
                &CancellationToken::new(),
            )
            .await
            .expect("baseline scan should succeed");

        // A plain file (5 bytes).
        let file_path = dir.path().join("scan_file.txt");
        std::fs::write(&file_path, b"12345").unwrap();
        backend
            .move_to_trash(&Location::Local(file_path), true, &CancellationToken::new())
            .await
            .expect("fallback move of file should succeed");

        // A directory containing a nested file (7 bytes) — exercises the
        // recursive-size path (calculate_dir_size_recursive reuse), not just
        // flat file sizes.
        let subdir = dir.path().join("scan_dir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("nested.txt"), b"1234567").unwrap();
        backend
            .move_to_trash(&Location::Local(subdir), true, &CancellationToken::new())
            .await
            .expect("fallback move of directory should succeed");

        let (after_count, after_size) = backend
            .scan_trash(
                std::slice::from_ref(&volume_root),
                &CancellationToken::new(),
            )
            .await
            .expect("scan after trashing should succeed");

        assert_eq!(
            after_count,
            before_count + 2,
            "should count both the file and the directory as one item each"
        );
        assert_eq!(
            after_size,
            before_size + 12,
            "5 (file) + 7 (nested file inside trashed dir) = 12"
        );
    }
}

#[cfg(all(test, unix))]
mod symlink_tests {
    use super::*;
    use crate::backend::FilesystemBackend;
    use crate::model::{LinkKind, Location};
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_regular_file_is_not_symlink() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("plain.txt"), "hi").unwrap();

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "plain.txt").unwrap();
        assert!(!e.is_symlink);
        assert_eq!(e.link_kind, None);
        assert_eq!(e.link_target, None);
    }

    #[tokio::test]
    async fn test_symlink_to_file_detected() {
        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "content").unwrap();
        std::os::unix::fs::symlink(&target, dir.path().join("link.txt")).unwrap();

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "link.txt").unwrap();
        assert!(e.is_symlink);
        assert_eq!(e.link_kind, Some(LinkKind::Symlink));
        assert!(e.link_target.is_some());
        assert!(!e.is_dir, "symlink to file should not be is_dir");
    }

    #[tokio::test]
    async fn test_symlink_to_dir_is_navigable() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("real_dir");
        std::fs::create_dir(&real).unwrap();
        std::os::unix::fs::symlink(&real, dir.path().join("link_dir")).unwrap();

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "link_dir").unwrap();
        assert!(e.is_symlink);
        assert!(
            e.is_dir,
            "symlink to directory must have is_dir=true for navigation"
        );
    }

    #[tokio::test]
    async fn test_broken_symlink_not_navigable() {
        let dir = tempfile::TempDir::new().unwrap();
        std::os::unix::fs::symlink("/nonexistent/path", dir.path().join("broken")).unwrap();

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "broken").unwrap();
        assert!(e.is_symlink);
        assert!(!e.is_dir, "broken symlink must not be navigable");
        assert!(
            e.link_target.is_some(),
            "broken symlink should still store its target path"
        );
    }
}

// Windows equivalent of the `unix` symlink_tests above (Phase M gap: navigability was
// only regression-tested on Unix). Directory symlinks require Developer Mode or admin
// privileges to create, so those tests skip (not fail) when unavailable; junctions never
// require elevation, so that test always runs.
#[cfg(all(test, windows))]
mod windows_symlink_tests {
    use super::*;
    use crate::backend::FilesystemBackend;
    use crate::model::Location;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn test_symlink_to_dir_is_navigable_windows() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("real_dir");
        std::fs::create_dir(&real).unwrap();
        if std::os::windows::fs::symlink_dir(&real, dir.path().join("link_dir")).is_err() {
            eprintln!(
                "skipping: creating a directory symlink requires Developer Mode or admin privileges"
            );
            return;
        }

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "link_dir").unwrap();
        assert!(e.is_symlink);
        assert!(
            e.is_dir,
            "symlink to directory must have is_dir=true for navigation"
        );
    }

    #[tokio::test]
    async fn test_junction_to_dir_is_navigable() {
        let dir = tempfile::TempDir::new().unwrap();
        let real = dir.path().join("real_dir");
        std::fs::create_dir(&real).unwrap();
        let link = dir.path().join("junction_link");

        // /D disables AutoRun so Clink (or similar shell extensions) can't inject and
        // pollute the exit code — same workaround as the Create Link job (line ~568).
        let status = std::process::Command::new("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&real)
            .status()
            .expect("failed to invoke mklink");
        assert!(
            status.success(),
            "mklink /J should not require elevated privileges"
        );

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "junction_link").unwrap();
        assert!(
            e.is_symlink,
            "Windows reparse points (junctions) are reported via is_symlink"
        );
        assert!(
            e.is_dir,
            "junction to directory must have is_dir=true for navigation"
        );
    }

    #[tokio::test]
    async fn test_broken_symlink_not_navigable_windows() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing = dir.path().join("does_not_exist");
        if std::os::windows::fs::symlink_dir(&missing, dir.path().join("broken")).is_err() {
            eprintln!(
                "skipping: creating a directory symlink requires Developer Mode or admin privileges"
            );
            return;
        }

        let entries = LocalFilesystemBackend::new()
            .read_directory(
                &Location::Local(dir.path().to_path_buf()),
                &CancellationToken::new(),
            )
            .await
            .unwrap();

        let e = entries.iter().find(|e| e.name == "broken").unwrap();
        assert!(e.is_symlink);
        assert!(!e.is_dir, "broken symlink must not be navigable");
    }
}
