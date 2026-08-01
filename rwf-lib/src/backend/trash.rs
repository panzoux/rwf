//! OS trash integration (Phase 7.7).
//!
//! Plain synchronous functions, same shape as `volume_info.rs`: callers
//! (`LocalFilesystemBackend`) wrap these in `tokio::task::spawn_blocking`.
//! No `unsafe` in this module — all OS interop goes through the `trash`
//! crate, which handles Windows Recycle Bin / macOS NSFileManager trash /
//! Linux FreeDesktop.org Trash spec internally.

use crate::model::{Location, TrashLocation, TrashRecord};
use anyhow::{Context, Result};
use std::path::Path;
use std::time::SystemTime;

/// Move `path` to the OS trash, returning a record with enough detail to
/// restore it later. `force_fallback` skips the OS trash call entirely and
/// goes straight to the `.rwf-trash` sidecar directory (see
/// `TrashConfig.force_fallback`). Also falls back automatically if the OS
/// trash call itself fails.
pub fn move_to_trash_sync(path: &Path, force_fallback: bool) -> Result<TrashRecord> {
    let metadata =
        std::fs::symlink_metadata(path).context("failed to read metadata before trashing")?;
    let size = metadata.len();
    let modified = metadata.modified().unwrap_or_else(|_| SystemTime::now());
    let original = Location::Local(path.to_path_buf());

    if !force_fallback && trash::delete(path).is_ok() {
        let trash_location = os_trash_entry_for(path).unwrap_or(TrashLocation::Untracked);
        return Ok(TrashRecord {
            original,
            trash_location,
            size,
            modified,
        });
    }

    fallback_move_to_trash(path, size, modified)
}

/// Look up the OS trash entry that `trash::delete(path)` just created, by
/// matching on parent directory + file name (the crate's `delete()` doesn't
/// return the created `TrashItem` directly, so we look it up).
#[cfg(any(target_os = "windows", target_os = "linux"))]
fn os_trash_entry_for(path: &Path) -> Option<TrashLocation> {
    let parent = path.parent()?.to_path_buf();
    let name = path.file_name()?.to_os_string();
    let mut matches: Vec<_> = trash::os_limited::list()
        .ok()?
        .into_iter()
        .filter(|item| item.original_parent == parent && item.name == name)
        .collect();
    matches.sort_by_key(|item| item.time_deleted);
    let item = matches.pop()?;
    Some(TrashLocation::OsManaged {
        id: item.id,
        name: item.name,
        original_parent: item.original_parent,
        time_deleted: item.time_deleted,
    })
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn os_trash_entry_for(_path: &Path) -> Option<TrashLocation> {
    None
}

fn fallback_move_to_trash(_path: &Path, _size: u64, _modified: SystemTime) -> Result<TrashRecord> {
    anyhow::bail!("not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_move_to_trash_removes_source_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("doomed.txt");
        std::fs::write(&file_path, b"gone soon").unwrap();

        let record = move_to_trash_sync(&file_path, false).expect("move to trash should succeed");

        assert!(
            !file_path.exists(),
            "source file must be gone after trashing"
        );
        assert_eq!(record.original, Location::Local(file_path));
        assert_eq!(record.size, 9);
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn test_move_to_trash_is_os_managed_on_windows_and_linux() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("tracked.txt");
        std::fs::write(&file_path, b"x").unwrap();

        let record = move_to_trash_sync(&file_path, false).expect("move to trash should succeed");

        assert!(
            matches!(record.trash_location, TrashLocation::OsManaged { .. }),
            "expected OsManaged, got {:?}",
            record.trash_location
        );

        // Cleanup: purge it so the test doesn't leave junk in the real
        // Recycle Bin / trash across runs.
        if let TrashLocation::OsManaged {
            id,
            name,
            original_parent,
            time_deleted,
        } = record.trash_location
        {
            let item = trash::TrashItem {
                id,
                name,
                original_parent,
                time_deleted,
            };
            let _ = trash::os_limited::purge_all(std::iter::once(item));
        }
    }

    #[test]
    fn test_move_to_trash_force_fallback_skips_os_trash() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("forced_fallback.txt");
        std::fs::write(&file_path, b"x").unwrap();

        let record = move_to_trash_sync(&file_path, true).expect("move to trash should succeed");

        assert!(
            matches!(record.trash_location, TrashLocation::Fallback { .. }),
            "force_fallback=true should always use the fallback tier, got {:?}",
            record.trash_location
        );
    }
}
