//! RAR through `unrar-ng` (the official UnRAR source, statically linked).
//!
//! Linux / macOS only: their 7z.so builds carry no RAR decoder (Homebrew disables
//! it, Debian ships it as a separate Codecs/Rar.so that crashes without help, see
//! plan/8.11 §2.9), while Windows' 7z.dll extracts RAR itself.

use super::listing::{direct_children, inner_key, relative_to, FlatItem};
use super::ArchiveHandler;
use crate::model::{FileEntry, Location};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use tokio_util::sync::CancellationToken;

/// A RAR header's MS-DOS time is local wall-clock time.
fn dos_time_to_system_time(t: u32) -> Option<SystemTime> {
    use chrono::TimeZone;
    let date = chrono::NaiveDate::from_ymd_opt(
        1980 + ((t >> 25) & 0x7f) as i32,
        (t >> 21) & 0x0f,
        (t >> 16) & 0x1f,
    )?;
    let dt = date.and_hms_opt((t >> 11) & 0x1f, (t >> 5) & 0x3f, (t & 0x1f) * 2)?;
    Some(chrono::Local.from_local_datetime(&dt).earliest()?.into())
}

fn entry_path(h: &unrar_ng::FileHeader) -> String {
    rwf_7zlib::normalize_item_path(&h.filename.to_string_lossy())
}

fn open_err(archive: &Path) -> String {
    format!("cannot open RAR archive {}", archive.display())
}

/// The direct children of `inner` in the archive.
pub fn list(archive: &Path, inner: &Path, cancel: &CancellationToken) -> Result<Vec<FileEntry>> {
    let listing = unrar_ng::Archive::new(archive)
        .open_for_listing()
        .with_context(|| open_err(archive))?;
    let mut items = Vec::new();
    for h in listing {
        if cancel.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let h = h.with_context(|| open_err(archive))?;
        items.push(FlatItem {
            path: entry_path(&h),
            is_dir: h.is_directory(),
            size: h.unpacked_size,
            modified: dos_time_to_system_time(h.file_time),
        });
    }
    Ok(direct_children(&items, archive, inner))
}

/// Walk the archive once; `dest_for(path, is_dir)` says where each entry goes.
fn process(
    archive: &Path,
    cancel: &CancellationToken,
    mut dest_for: impl FnMut(&str, bool) -> Option<PathBuf>,
) -> Result<usize> {
    let mut cursor = unrar_ng::Archive::new(archive)
        .open_for_processing()
        .with_context(|| open_err(archive))?;
    let mut written = 0;
    while let Some(header) = cursor.read_header().with_context(|| open_err(archive))? {
        if cancel.is_cancelled() {
            anyhow::bail!("Operation cancelled");
        }
        let path = entry_path(header.entry());
        let is_dir = header.entry().is_directory();
        cursor = match dest_for(&path, is_dir) {
            Some(dest) if is_dir => {
                std::fs::create_dir_all(&dest)?;
                header.skip()?
            }
            Some(dest) => {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                written += 1;
                header
                    .extract_to(&dest)
                    .with_context(|| format!("unrar could not extract {path}"))?
            }
            None => header.skip()?,
        };
    }
    Ok(written)
}

/// Extract the single file at `inner` to the file path `dest`.
pub fn extract_file(
    archive: &Path,
    inner: &Path,
    dest: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    let key = inner_key(inner);
    let n = process(archive, cancel, |path, is_dir| {
        (!is_dir && path == key).then(|| dest.to_path_buf())
    })?;
    if n == 0 {
        anyhow::bail!("File not found in RAR archive: {key}");
    }
    Ok(())
}

/// Extract everything below `inner` (the whole archive for `None`) into `dest`.
pub fn extract_tree(
    archive: &Path,
    inner: Option<&Path>,
    dest: &Path,
    cancel: &CancellationToken,
) -> Result<()> {
    let key = inner.map(inner_key).unwrap_or_default();
    std::fs::create_dir_all(dest)?;
    process(archive, cancel, |path, _| {
        let rel = relative_to(path, &key)?;
        let target = rwf_7zlib::safe_join(dest, rel);
        if target.is_none() {
            tracing::warn!("skipping RAR entry outside the destination: {path}");
        }
        target
    })?;
    Ok(())
}

/// RAR through `unrar-ng` (Linux / macOS, where 7z.so has no RAR decoder).
pub struct RarArchiveHandler;

impl RarArchiveHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RarArchiveHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl ArchiveHandler for RarArchiveHandler {
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
                Location::Local(path) => list(path, inner_path, cancel_token),
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
                Location::Local(file) => extract_file(file, inner_path, dest_path, cancel_token),
                _ => anyhow::bail!("Only local archives are supported"),
            },
            _ => anyhow::bail!("Invalid source/dest combination for RAR extraction"),
        }
    }

    async fn extract_all(
        &self,
        archive: &Location,
        dest: &Location,
        cancel_token: &CancellationToken,
    ) -> Result<()> {
        match (archive, dest) {
            (Location::Local(file), Location::Local(dest_path)) => {
                extract_tree(file, None, dest_path, cancel_token)
            }
            (
                Location::Archive {
                    archive_path,
                    inner_path,
                },
                Location::Local(dest_path),
            ) => match archive_path.as_ref() {
                Location::Local(file) => {
                    extract_tree(file, Some(inner_path), dest_path, cancel_token)
                }
                _ => anyhow::bail!("Only local archives are supported"),
            },
            _ => anyhow::bail!("Invalid archive/dest combination"),
        }
    }

    async fn create_archive(
        &self,
        _: &[Location],
        _: &Location,
        _: &CancellationToken,
    ) -> Result<()> {
        anyhow::bail!(
            "RAR is a proprietary format — rwf cannot create RAR archives. Use ZIP or 7z instead."
        )
    }

    fn is_archive(&self, filename: &str) -> bool {
        filename.to_lowercase().ends_with(".rar")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dos_time_decodes_fields() {
        // 2010-01-08 23:48:20 local
        let t = ((2010 - 1980) << 25) | (1 << 21) | (8 << 16) | (23 << 11) | (48 << 5) | 10;
        let st = dos_time_to_system_time(t).unwrap();
        let local: chrono::DateTime<chrono::Local> = st.into();
        assert_eq!(
            local.format("%Y-%m-%d %H:%M:%S").to_string(),
            "2010-01-08 23:48:20"
        );
        assert_eq!(dos_time_to_system_time(0), None); // month 0 is invalid
    }

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("test_data/editd169e.rar")
    }

    #[test]
    fn lists_and_extracts_the_fixture() {
        let cancel = CancellationToken::new();
        let v = list(&fixture(), Path::new(""), &cancel).unwrap();
        assert_eq!(v.len(), 6);
        let dir = tempfile::tempdir().unwrap();
        extract_tree(&fixture(), None, dir.path(), &cancel).unwrap();
        assert_eq!(
            std::fs::metadata(dir.path().join("editdisk.exe"))
                .unwrap()
                .len(),
            560_640
        );
        let one = dir.path().join("one.txt");
        extract_file(&fixture(), Path::new("readme.txt"), &one, &cancel).unwrap();
        assert_eq!(std::fs::metadata(&one).unwrap().len(), 792);
        assert!(extract_file(&fixture(), Path::new("nope"), &one, &cancel).is_err());
    }
}
