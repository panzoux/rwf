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

    fallback_move_to_trash(path, &volume_root(path).join(".rwf-trash"), size, modified)
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

#[derive(serde::Serialize, serde::Deserialize)]
struct FallbackMeta {
    original: std::path::PathBuf,
    trashed_at: i64,
}

/// The topmost ancestor of `path` — the drive root on Windows, `/` on Unix.
/// Used as the anchor for the `.rwf-trash` sidecar directory, matching the
/// original spec's "create it at the volume root" fallback strategy, and
/// matching how `purge_fallback_dirs_sync` (a later task) and
/// `Action::EmptyTrash` locate `.rwf-trash` dirs to sweep — if this ever
/// anchored somewhere other than the true volume root, EmptyTrash would
/// silently stop finding fallback-trashed files.
fn volume_root(path: &Path) -> std::path::PathBuf {
    path.ancestors().last().unwrap_or(path).to_path_buf()
}

/// Restore a previously trashed item back to `record.original`.
pub fn restore_from_trash_sync(record: &TrashRecord) -> Result<()> {
    match &record.trash_location {
        TrashLocation::OsManaged {
            id,
            name,
            original_parent,
            time_deleted,
        } => restore_os_managed(id, name, original_parent, *time_deleted),
        TrashLocation::Fallback { trash_path } => restore_fallback(trash_path),
        TrashLocation::Untracked => {
            anyhow::bail!(
                "this item was trashed without restore tracking on this platform; \
                 restore it manually from the OS trash"
            )
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn restore_os_managed(
    id: &std::ffi::OsString,
    name: &std::ffi::OsString,
    original_parent: &Path,
    time_deleted: i64,
) -> Result<()> {
    let item = trash::TrashItem {
        id: id.clone(),
        name: name.clone(),
        original_parent: original_parent.to_path_buf(),
        time_deleted,
    };
    trash::os_limited::restore_all(std::iter::once(item))
        .context("failed to restore item from trash")
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn restore_os_managed(
    _id: &std::ffi::OsString,
    _name: &std::ffi::OsString,
    _original_parent: &Path,
    _time_deleted: i64,
) -> Result<()> {
    anyhow::bail!("restore from OS trash is not supported on this platform")
}

/// The `.rwf-meta.json` sidecar path for a fallback-trashed item, derived
/// via `OsString` concatenation (not `format!`/`to_string_lossy`) so it
/// round-trips non-UTF-8 paths losslessly. Shared by `fallback_move_to_trash`
/// (which writes it) and `restore_fallback` (which reads it) so the naming
/// convention can't drift between the two.
fn sidecar_meta_path(trash_path: &Path) -> std::path::PathBuf {
    let mut meta_os = trash_path.as_os_str().to_os_string();
    meta_os.push(".rwf-meta.json");
    std::path::PathBuf::from(meta_os)
}

fn fallback_move_to_trash(
    path: &Path,
    trash_dir: &Path,
    size: u64,
    modified: SystemTime,
) -> Result<TrashRecord> {
    std::fs::create_dir_all(trash_dir).context("failed to create .rwf-trash directory")?;

    let file_name = path
        .file_name()
        .context("trash target has no file name")?
        .to_string_lossy();
    let unique_name = format!("{}-{}", uuid::Uuid::new_v4(), file_name);
    let trash_path = trash_dir.join(&unique_name);

    std::fs::rename(path, &trash_path).context("failed to move file into .rwf-trash fallback")?;

    let meta = FallbackMeta {
        original: path.to_path_buf(),
        trashed_at: chrono::Utc::now().timestamp(),
    };
    let meta_path = sidecar_meta_path(&trash_path);
    std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).context("failed to serialize .rwf-trash metadata")?,
    )
    .context("failed to write .rwf-trash metadata")?;

    Ok(TrashRecord {
        original: Location::Local(path.to_path_buf()),
        trash_location: TrashLocation::Fallback { trash_path },
        size,
        modified,
    })
}

/// Restore a `.rwf-trash`-fallback-trashed item back to its original path.
fn restore_fallback(trash_path: &Path) -> Result<()> {
    let meta_path = sidecar_meta_path(trash_path);

    let meta_json =
        std::fs::read_to_string(&meta_path).context("failed to read .rwf-trash metadata")?;
    let meta: FallbackMeta =
        serde_json::from_str(&meta_json).context("failed to parse .rwf-trash metadata")?;

    if let Some(parent) = meta.original.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::rename(trash_path, &meta.original)
        .context("failed to restore file from .rwf-trash fallback")?;
    std::fs::remove_file(&meta_path).ok();
    Ok(())
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

        // Cleanup: this test exercises the real move_to_trash_sync public
        // entry point, which computes the true volume root — remove the
        // fallback file + its metadata sidecar so no residue is left on the
        // real drive root across test runs.
        if let TrashLocation::Fallback { trash_path } = &record.trash_location {
            let _ = std::fs::remove_file(sidecar_meta_path(trash_path));
            let _ = std::fs::remove_file(trash_path);
        }
    }

    #[test]
    fn test_fallback_move_to_trash_creates_sidecar_and_metadata() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("fallback_me.txt");
        std::fs::write(&file_path, b"12345").unwrap();

        let record = fallback_move_to_trash(
            &file_path,
            &dir.path().join(".rwf-trash"),
            5,
            SystemTime::now(),
        )
        .expect("fallback move should succeed");

        assert!(!file_path.exists());
        let trash_path = match &record.trash_location {
            TrashLocation::Fallback { trash_path } => trash_path.clone(),
            other => panic!("expected Fallback, got {other:?}"),
        };
        assert!(
            trash_path.exists(),
            "trashed file should exist at trash_path"
        );
        assert!(
            trash_path.starts_with(dir.path().join(".rwf-trash")),
            "trash_path should be under .rwf-trash: {trash_path:?}"
        );

        assert!(
            sidecar_meta_path(&trash_path).exists(),
            "metadata sidecar should exist"
        );
    }

    #[test]
    fn test_restore_fallback_moves_file_back() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("roundtrip.txt");
        std::fs::write(&file_path, b"back again").unwrap();

        let record = fallback_move_to_trash(
            &file_path,
            &dir.path().join(".rwf-trash"),
            10,
            SystemTime::now(),
        )
        .unwrap();
        let trash_path = match &record.trash_location {
            TrashLocation::Fallback { trash_path } => trash_path.clone(),
            other => panic!("expected Fallback, got {other:?}"),
        };

        restore_fallback(&trash_path).expect("restore should succeed");

        assert!(
            file_path.exists(),
            "file should be back at its original path"
        );
        assert!(!trash_path.exists(), "trashed copy should be gone");
        assert_eq!(std::fs::read(&file_path).unwrap(), b"back again");
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn test_restore_from_trash_os_managed_round_trip() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("restore_me.txt");
        std::fs::write(&file_path, b"restore me").unwrap();

        let record = move_to_trash_sync(&file_path, false).expect("trash should succeed");
        assert!(!file_path.exists());

        restore_from_trash_sync(&record).expect("restore should succeed");

        assert!(file_path.exists(), "file should be back after restore");
        assert_eq!(std::fs::read(&file_path).unwrap(), b"restore me");
    }

    #[test]
    fn test_restore_from_trash_untracked_returns_error() {
        let record = TrashRecord {
            original: Location::Local(std::path::PathBuf::from("C:/nowhere/x.txt")),
            trash_location: TrashLocation::Untracked,
            size: 0,
            modified: SystemTime::now(),
        };

        let err = restore_from_trash_sync(&record).expect_err("Untracked must not be restorable");
        assert!(err.to_string().contains("manually"));
    }
}
