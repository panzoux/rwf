//! ISO 9660 images (read-only), using the `iso9660` crate.

use super::ArchiveHandler;
use crate::model::{FileEntry, Location};
use anyhow::Result;
use tokio_util::sync::CancellationToken;

/// ISO 9660 archive handler (read-only, using the iso9660 crate)
pub struct IsoArchiveHandler;

impl IsoArchiveHandler {
    /// Create a new ISO archive handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for IsoArchiveHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ArchiveHandler for IsoArchiveHandler {
    async fn list_entries(
        &self,
        location: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<Vec<FileEntry>> {
        match location {
            Location::Archive {
                archive_path,
                inner_path,
            } => match archive_path.as_ref() {
                Location::Local(path) => list_iso_entries(path, inner_path, cancel_token),
                _ => anyhow::bail!("Only local archives are supported"),
            },
            _ => anyhow::bail!("Expected Archive location"),
        }
    }

    async fn extract_file(
        &self,
        source: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match (source, dest) {
            (
                Location::Archive {
                    archive_path,
                    inner_path,
                },
                Location::Local(dest_path),
            ) => match archive_path.as_ref() {
                Location::Local(archive_file) => {
                    extract_iso_single_file(archive_file, inner_path, dest_path, cancel_token)
                }
                _ => anyhow::bail!("Only local archives are supported"),
            },
            _ => anyhow::bail!("Invalid source/dest combination for ISO extraction"),
        }
    }

    async fn extract_all(
        &self,
        archive: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match (archive, dest) {
            (Location::Local(archive_path), Location::Local(dest_path)) => {
                std::fs::create_dir_all(dest_path)?;
                let iso = open_iso(archive_path)?;
                extract_iso_dir_recursive(&iso, "/", dest_path, cancel_token)
            }
            (
                Location::Archive {
                    archive_path,
                    inner_path,
                },
                Location::Local(dest_path),
            ) => match archive_path.as_ref() {
                Location::Local(archive_file) => {
                    let iso_path = format!("/{}", inner_path.to_string_lossy().replace('\\', "/"));
                    std::fs::create_dir_all(dest_path)?;
                    let iso = open_iso(archive_file)?;
                    extract_iso_dir_recursive(&iso, &iso_path, dest_path, cancel_token)
                }
                _ => anyhow::bail!("Only local archives are supported"),
            },
            _ => anyhow::bail!("Invalid archive/dest combination"),
        }
    }

    async fn create_archive(
        &self,
        _sources: &[Location],
        _dest: &Location,
        _cancel_token: &CancellationToken,
    ) -> Result<()> {
        anyhow::bail!("ISO images are read-only disc images — rwf cannot create ISO files.")
    }

    fn is_archive(&self, filename: &str) -> bool {
        filename.to_lowercase().ends_with(".iso")
    }
}

// ── ISO helpers (synchronous, no .await — ISO9660<T> uses Rc internally) ────

fn open_iso(path: &std::path::Path) -> Result<iso9660::ISO9660<std::io::BufReader<std::fs::File>>> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    iso9660::ISO9660::new(file).map_err(|e| anyhow::anyhow!("Failed to open ISO: {}", e))
}

fn list_iso_entries(
    archive_path: &std::path::Path,
    inner_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<Vec<FileEntry>> {
    use std::time::SystemTime;

    let iso = open_iso(archive_path)?;
    let iso_path = {
        let s = inner_path.to_string_lossy();
        if s.is_empty() {
            "/".to_string()
        } else {
            format!("/{}", s.replace('\\', "/"))
        }
    };

    let dir_entry = iso
        .open(&iso_path)
        .map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?;
    let dir = match dir_entry {
        Some(iso9660::DirectoryEntry::Directory(d)) => d,
        Some(iso9660::DirectoryEntry::File(_)) => anyhow::bail!("Path is a file, not a directory"),
        None => anyhow::bail!("Directory not found in ISO: {}", iso_path),
    };

    let mut entries = Vec::new();
    for entry in dir.contents() {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let entry = entry.map_err(|e| anyhow::anyhow!("ISO entry error: {}", e))?;
        let raw_name = entry.identifier();
        // Skip dot/dotdot (identifiers \x00 and \x01)
        if raw_name == "\x00" || raw_name == "\x01" {
            continue;
        }
        // ISO 9660 file identifiers have ";1" version suffix — strip it
        let name = raw_name.trim_end_matches(";1").to_string();
        if name.is_empty() {
            continue;
        }

        let is_dir = matches!(entry, iso9660::DirectoryEntry::Directory(_));
        let size = match &entry {
            iso9660::DirectoryEntry::File(f) => f.size() as u64,
            iso9660::DirectoryEntry::Directory(_) => 0,
        };
        let entry_inner = if inner_path.components().count() == 0 {
            std::path::PathBuf::from(&name)
        } else {
            inner_path.join(&name)
        };

        entries.push(FileEntry {
            name,
            location: Location::Archive {
                archive_path: Box::new(Location::Local(archive_path.to_path_buf())),
                inner_path: entry_inner,
            },
            size,
            is_dir,
            is_hidden: false,
            modified: SystemTime::now(),
            marked: false,
            calculated_size: None,
            is_symlink: false,
            link_target: None,
            link_kind: None,
        });
    }
    Ok(entries)
}

fn extract_iso_single_file(
    archive_path: &std::path::Path,
    inner_path: &std::path::Path,
    dest_path: &std::path::Path,
    _cancel_token: &CancellationToken,
) -> Result<()> {
    let iso = open_iso(archive_path)?;
    let iso_path = format!("/{}", inner_path.to_string_lossy().replace('\\', "/"));
    match iso
        .open(&iso_path)
        .map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?
    {
        Some(iso9660::DirectoryEntry::File(f)) => {
            if let Some(p) = dest_path.parent() {
                std::fs::create_dir_all(p)?;
            }
            let mut reader = f.read();
            let mut out = std::fs::File::create(dest_path)?;
            std::io::copy(&mut reader, &mut out)?;
            Ok(())
        }
        Some(iso9660::DirectoryEntry::Directory(_)) => anyhow::bail!("Path is a directory"),
        None => anyhow::bail!("File not found in ISO: {}", iso_path),
    }
}

fn extract_iso_dir_recursive(
    iso: &iso9660::ISO9660<std::io::BufReader<std::fs::File>>,
    iso_path: &str,
    dest_path: &std::path::Path,
    cancel_token: &CancellationToken,
) -> Result<()> {
    let dir_entry = iso
        .open(iso_path)
        .map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?;
    let dir = match dir_entry {
        Some(iso9660::DirectoryEntry::Directory(d)) => d,
        _ => return Ok(()),
    };

    // Collect names first to avoid borrow issues
    let mut items: Vec<(String, bool)> = Vec::new();
    for entry in dir.contents() {
        let entry = entry.map_err(|e| anyhow::anyhow!("ISO entry error: {}", e))?;
        let raw = entry.identifier();
        if raw == "\x00" || raw == "\x01" {
            continue;
        }
        let name = raw.trim_end_matches(";1").to_string();
        if name.is_empty() {
            continue;
        }
        let is_dir = matches!(entry, iso9660::DirectoryEntry::Directory(_));
        items.push((name, is_dir));
    }

    for (name, is_dir) in items {
        if cancel_token.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let out_path = dest_path.join(&name);
        let child_iso_path = if iso_path == "/" {
            format!("/{}", name)
        } else {
            format!("{}/{}", iso_path.trim_end_matches('/'), name)
        };
        if is_dir {
            std::fs::create_dir_all(&out_path)?;
            extract_iso_dir_recursive(iso, &child_iso_path, &out_path, cancel_token)?;
        } else if let Some(iso9660::DirectoryEntry::File(f)) = iso
            .open(&child_iso_path)
            .map_err(|e| anyhow::anyhow!("ISO navigate error: {}", e))?
        {
            let mut reader = f.read();
            let mut out = std::fs::File::create(&out_path)?;
            std::io::copy(&mut reader, &mut out)?;
        }
    }
    Ok(())
}
