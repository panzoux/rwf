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
pub(crate) fn volume_root(path: &Path) -> std::path::PathBuf {
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
        TrashLocation::Fallback { trash_path, .. } => restore_fallback(trash_path),
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

/// Purge the OS-managed trash (Windows Recycle Bin / Linux FreeDesktop
/// trash via the `trash` crate). `older_than_days` restricts the purge to
/// items trashed longer ago than that (`None` purges everything). Returns
/// `0` on platforms where `os_limited` isn't available (macOS) rather than
/// erroring — there's nothing tracked there to purge.
pub fn purge_os_trash_sync(older_than_days: Option<u32>) -> Result<usize> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let cutoff =
            older_than_days.map(|days| chrono::Utc::now().timestamp() - (days as i64) * 86_400);
        let items: Vec<_> = trash::os_limited::list()
            .context("failed to list trash items")?
            .into_iter()
            .filter(|item| cutoff.is_none_or(|c| item.time_deleted < c))
            .collect();
        let count = items.len();
        if count > 0 {
            trash::os_limited::purge_all(items).context("failed to purge trash items")?;
        }
        Ok(count)
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        let _ = older_than_days;
        Ok(0)
    }
}

/// Non-destructively count items and sum byte sizes in the OS-managed trash.
/// Returns `(0, 0)` on platforms where `os_limited` isn't available (macOS).
///
/// Byte size is only summable for items the `trash` crate reports as
/// `TrashItemSize::Bytes` (individual files). Directories are reported as
/// `TrashItemSize::Entries(non_recursive_count)` — the crate has no API for
/// a directory's true recursive byte size once inside OS-managed trash
/// storage, so directory items count toward `count` but contribute `0` to
/// `total_size`. This under-counts total size when the trash holds trashed
/// directories; it never over-counts.
pub fn scan_os_trash_sync() -> Result<(usize, u64)> {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let items = trash::os_limited::list().context("failed to list trash items")?;
        let count = items.len();
        let total_size = items
            .iter()
            .filter_map(|item| trash::os_limited::metadata(item).ok())
            .filter_map(|meta| meta.size.size())
            .sum();
        Ok((count, total_size))
    }
    #[cfg(not(any(target_os = "windows", target_os = "linux")))]
    {
        Ok((0, 0))
    }
}

/// Purge every `.rwf-trash` fallback directory found directly under each of
/// `roots` (bounded sweep — no unbounded filesystem scanning). Returns the
/// number of payload files/dirs removed (metadata sidecars aren't counted
/// separately).
pub fn purge_fallback_dirs_sync(roots: &[std::path::PathBuf]) -> Result<usize> {
    let mut purged = 0usize;
    for root in roots {
        let trash_dir = root.join(".rwf-trash");
        if !trash_dir.exists() {
            continue;
        }
        let entries =
            std::fs::read_dir(&trash_dir).context("failed to read .rwf-trash directory")?;
        for entry in entries {
            let entry = entry.context("failed to read .rwf-trash entry")?;
            let path = entry.path();
            let is_meta = path.to_string_lossy().ends_with(".rwf-meta.json");
            if is_meta {
                std::fs::remove_file(&path).ok();
                continue;
            }
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path).is_ok()
            } else {
                std::fs::remove_file(&path).is_ok()
            };
            if removed {
                purged += 1;
            }
        }
    }
    Ok(purged)
}

/// Non-destructively list every trashed item (OS-managed + every `.rwf-trash`
/// fallback dir under `roots`), for the trash-browser UI (Phase 7.7 Task 16).
/// Sorted newest-deleted-first. Directory sizes are recomputed recursively
/// here rather than trusted from move-time (`move_to_trash_sync` stores a
/// directory's raw `symlink_metadata().len()`, which is not its recursive
/// size on any platform).
pub fn list_trash_sync(fallback_roots: &[std::path::PathBuf]) -> Result<Vec<TrashRecord>> {
    let mut records = Vec::new();

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    {
        let items = trash::os_limited::list().context("failed to list trash items")?;
        for item in items {
            let size = trash::os_limited::metadata(&item)
                .ok()
                .and_then(|meta| meta.size.size())
                .unwrap_or(0);
            let original = Location::Local(item.original_parent.join(&item.name));
            records.push(TrashRecord {
                original,
                trash_location: TrashLocation::OsManaged {
                    id: item.id,
                    name: item.name,
                    original_parent: item.original_parent,
                    time_deleted: item.time_deleted,
                },
                size,
                // Original mtime isn't exposed by the OS trash listing API
                // (`TrashItemMetadata` only has size); unused by
                // `restore_from_trash_sync`, which restores by
                // `trash_location` alone.
                modified: SystemTime::UNIX_EPOCH,
            });
        }
    }

    for root in fallback_roots {
        let trash_dir = root.join(".rwf-trash");
        if !trash_dir.exists() {
            continue;
        }
        let entries =
            std::fs::read_dir(&trash_dir).context("failed to read .rwf-trash directory")?;
        for entry in entries {
            let entry = entry.context("failed to read .rwf-trash entry")?;
            let path = entry.path();
            if path.to_string_lossy().ends_with(".rwf-meta.json") {
                continue;
            }
            let meta_path = sidecar_meta_path(&path);
            let Ok(meta_json) = std::fs::read_to_string(&meta_path) else {
                continue;
            };
            let Ok(meta) = serde_json::from_str::<FallbackMeta>(&meta_json) else {
                continue;
            };
            let metadata = entry
                .metadata()
                .context("failed to stat .rwf-trash entry")?;
            let size = if metadata.is_dir() {
                dir_size_sync(&path).unwrap_or(0)
            } else {
                metadata.len()
            };
            records.push(TrashRecord {
                original: Location::Local(meta.original),
                trash_location: TrashLocation::Fallback {
                    trash_path: path,
                    trashed_at: meta.trashed_at,
                },
                size,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            });
        }
    }

    records.sort_by_key(|r| {
        std::cmp::Reverse(match &r.trash_location {
            TrashLocation::OsManaged { time_deleted, .. } => *time_deleted,
            TrashLocation::Fallback { trashed_at, .. } => *trashed_at,
            TrashLocation::Untracked => 0,
        })
    });

    Ok(records)
}

/// Plain recursive directory size (bytes), for `list_trash_sync`'s fallback-dir
/// sizing. Best-effort: unreadable entries are skipped rather than failing the
/// whole listing.
fn dir_size_sync(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in std::fs::read_dir(path)?.flatten() {
        let entry_path = entry.path();
        if let Ok(metadata) = entry.metadata() {
            total += if metadata.is_dir() {
                dir_size_sync(&entry_path).unwrap_or(0)
            } else {
                metadata.len()
            };
        }
    }
    Ok(total)
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

    let trashed_at = chrono::Utc::now().timestamp();
    let meta = FallbackMeta {
        original: path.to_path_buf(),
        trashed_at,
    };
    let meta_path = sidecar_meta_path(&trash_path);
    std::fs::write(
        &meta_path,
        serde_json::to_string_pretty(&meta).context("failed to serialize .rwf-trash metadata")?,
    )
    .context("failed to write .rwf-trash metadata")?;

    Ok(TrashRecord {
        original: Location::Local(path.to_path_buf()),
        trash_location: TrashLocation::Fallback {
            trash_path,
            trashed_at,
        },
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
        if let TrashLocation::Fallback { trash_path, .. } = &record.trash_location {
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
            TrashLocation::Fallback { trash_path, .. } => trash_path.clone(),
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
            TrashLocation::Fallback { trash_path, .. } => trash_path.clone(),
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

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    #[ignore = "destructive: purge_os_trash_sync(None) empties the ENTIRE real OS trash, \
                not just this test's own item — only run deliberately with `cargo test -- \
                --ignored` in a disposable environment, never as part of routine `cargo test`"]
    fn test_purge_os_trash_purges_item() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("purge_me.txt");
        std::fs::write(&file_path, b"x").unwrap();
        move_to_trash_sync(&file_path, false).expect("trash should succeed");

        let purged = purge_os_trash_sync(None).expect("purge should succeed");

        assert!(
            purged >= 1,
            "expected at least the item we just trashed to be purged"
        );
        let still_present = trash::os_limited::list()
            .unwrap_or_default()
            .into_iter()
            .any(|item| item.original_parent == dir.path() && item.name == "purge_me.txt");
        assert!(
            !still_present,
            "item should no longer be listed after purge"
        );
    }

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[test]
    fn test_purge_os_trash_respects_age_cutoff() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("too_new_to_purge.txt");
        std::fs::write(&file_path, b"x").unwrap();
        let record = move_to_trash_sync(&file_path, false).expect("trash should succeed");

        // A huge cutoff (9999 days) means "only purge things older than ~27 years" —
        // nothing in any real trash could match that, so this is safe to run against
        // a real, populated OS trash: it should purge nothing at all.
        let purged = purge_os_trash_sync(Some(9999)).expect("purge should succeed");
        assert_eq!(purged, 0, "a huge age cutoff should purge nothing");

        // Confirm our freshly-trashed item specifically is still present.
        let still_present = trash::os_limited::list()
            .unwrap_or_default()
            .into_iter()
            .any(|item| item.original_parent == dir.path() && item.name == "too_new_to_purge.txt");
        assert!(
            still_present,
            "item should NOT have been purged by a huge age cutoff"
        );

        // Cleanup: purge only this one specific item (same pattern as the existing
        // test_move_to_trash_is_os_managed_on_windows_and_linux test), not a bulk sweep.
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
    fn test_purge_fallback_dirs_purges_item() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("fallback_purge_me.txt");
        std::fs::write(&file_path, b"x").unwrap();
        fallback_move_to_trash(
            &file_path,
            &dir.path().join(".rwf-trash"),
            1,
            SystemTime::now(),
        )
        .unwrap();

        let trash_dir = dir.path().join(".rwf-trash");
        assert!(trash_dir.exists());
        assert!(std::fs::read_dir(&trash_dir).unwrap().next().is_some());

        let purged = purge_fallback_dirs_sync(std::slice::from_ref(&dir.path().to_path_buf()))
            .expect("purge should succeed");

        assert!(purged >= 1);
        assert!(
            std::fs::read_dir(&trash_dir).unwrap().next().is_none(),
            ".rwf-trash should be empty after purge"
        );
    }

    #[test]
    fn test_purge_fallback_dirs_skips_roots_with_no_trash_dir() {
        let dir = TempDir::new().unwrap();
        let purged = purge_fallback_dirs_sync(std::slice::from_ref(&dir.path().to_path_buf()))
            .expect("purge should succeed even with nothing to purge");
        assert_eq!(purged, 0);
    }

    #[test]
    fn test_scan_os_trash_sync_counts_and_sums_a_freshly_trashed_file() {
        // Non-destructive (unlike purge_os_trash_sync), so this is safe to run against
        // a real, possibly-populated OS trash without #[ignore] — asserted as a delta
        // rather than an absolute count/size, same reasoning as
        // test_purge_os_trash_respects_age_cutoff above.
        let (before_count, before_size) = scan_os_trash_sync().expect("scan should succeed");

        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("scan_me.txt");
        let contents = b"twelve bytes";
        std::fs::write(&file_path, contents).unwrap();
        let record = move_to_trash_sync(&file_path, false).expect("trash should succeed");

        let (after_count, after_size) = scan_os_trash_sync().expect("scan should succeed");

        assert_eq!(
            after_count,
            before_count + 1,
            "scan should see the freshly trashed item"
        );
        assert_eq!(
            after_size,
            before_size + contents.len() as u64,
            "scan should sum the freshly trashed file's real byte size"
        );

        // Cleanup: purge only this specific item (same pattern as
        // test_move_to_trash_is_os_managed_on_windows_and_linux).
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
    fn test_list_trash_sync_lists_fallback_file_and_directory_with_sizes_and_original_paths() {
        let dir = TempDir::new().unwrap();
        let volume_root = dir.path().ancestors().last().unwrap().to_path_buf();
        let trash_dir = volume_root.join(".rwf-trash");

        let file_path = dir.path().join("list_me.txt");
        std::fs::write(&file_path, b"12345").unwrap(); // 5 bytes

        let subdir = dir.path().join("list_dir");
        std::fs::create_dir(&subdir).unwrap();
        std::fs::write(subdir.join("nested.txt"), b"1234567").unwrap(); // 7 bytes

        move_to_trash_sync(&file_path, true).expect("fallback move of file should succeed");
        move_to_trash_sync(&subdir, true).expect("fallback move of directory should succeed");

        let records =
            list_trash_sync(std::slice::from_ref(&volume_root)).expect("list should succeed");

        let file_record = records
            .iter()
            .find(|r| r.original == Location::Local(file_path.clone()))
            .expect("listed records should include the trashed file");
        assert_eq!(file_record.size, 5);
        assert!(matches!(
            file_record.trash_location,
            TrashLocation::Fallback { .. }
        ));

        let dir_record = records
            .iter()
            .find(|r| r.original == Location::Local(subdir.clone()))
            .expect("listed records should include the trashed directory");
        assert_eq!(
            dir_record.size, 7,
            "directory size should be the recursive sum of its contents"
        );

        // Cleanup: remove both trashed items + their sidecars from the real volume root.
        for record in [file_record, dir_record] {
            if let TrashLocation::Fallback { trash_path, .. } = &record.trash_location {
                let _ = std::fs::remove_file(sidecar_meta_path(trash_path));
                if trash_path.is_dir() {
                    let _ = std::fs::remove_dir_all(trash_path);
                } else {
                    let _ = std::fs::remove_file(trash_path);
                }
            }
        }
        let _ = std::fs::remove_dir(&trash_dir);
    }
}
