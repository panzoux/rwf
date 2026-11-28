//! Turn a flat list of archive item paths into the entries one directory level
//! shows, for the archive back ends that only hand out flat item lists (7-Zip,
//! unrar).

use crate::model::{FileEntry, Location};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// One item of an archive, as a back end reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatItem {
    /// `/`-separated path inside the archive, no leading `/`.
    pub path: String,
    pub is_dir: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

/// `inner_path` of a [`Location::Archive`] as a `/`-separated key without
/// leading or trailing `/` (`""` is the archive root).
pub fn inner_key(inner: &Path) -> String {
    inner
        .to_string_lossy()
        .replace('\\', "/")
        .split('/')
        .filter(|c| !c.is_empty() && *c != ".")
        .collect::<Vec<_>>()
        .join("/")
}

/// `relative` part of `path` below the directory `key`, or `None` if `path` is not
/// strictly inside it.
pub fn relative_to<'a>(path: &'a str, key: &str) -> Option<&'a str> {
    if key.is_empty() {
        return (!path.is_empty()).then_some(path);
    }
    let rest = path.strip_prefix(key)?.strip_prefix('/')?;
    (!rest.is_empty()).then_some(rest)
}

/// The direct children of `inner` in `items`. Directories that only appear as
/// part of a deeper path (archives without directory records) are synthesized.
pub fn direct_children(items: &[FlatItem], archive: &Path, inner: &Path) -> Vec<FileEntry> {
    let key = inner_key(inner);
    let mut order: Vec<String> = Vec::new();
    let mut found: HashMap<String, (bool, u64, Option<SystemTime>)> = HashMap::new();
    for item in items {
        let Some(rel) = relative_to(&item.path, &key) else {
            continue;
        };
        let (name, deeper) = match rel.split_once('/') {
            Some((first, _)) => (first, true),
            None => (rel, false),
        };
        let is_dir = deeper || item.is_dir;
        match found.get_mut(name) {
            Some(slot) => {
                if !deeper {
                    // The item's own record beats what a deeper path implied.
                    *slot = (is_dir, if is_dir { 0 } else { item.size }, item.modified);
                }
            }
            None => {
                order.push(name.to_string());
                let (size, modified) = if deeper {
                    (0, None)
                } else {
                    (if is_dir { 0 } else { item.size }, item.modified)
                };
                found.insert(name.to_string(), (is_dir, size, modified));
            }
        }
    }
    order
        .into_iter()
        .filter_map(|name| {
            let (is_dir, size, modified) = found.remove(&name)?;
            let inner_path = if key.is_empty() {
                PathBuf::from(&name)
            } else {
                inner.join(&name)
            };
            Some(FileEntry {
                is_hidden: name.starts_with('.'),
                name,
                location: Location::Archive {
                    archive_path: Box::new(Location::Local(archive.to_path_buf())),
                    inner_path,
                },
                size,
                is_dir,
                modified: modified.unwrap_or_else(SystemTime::now),
                marked: false,
                calculated_size: None,
                is_symlink: false,
                link_target: None,
                link_kind: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, is_dir: bool, size: u64) -> FlatItem {
        FlatItem {
            path: path.into(),
            is_dir,
            size,
            modified: Some(SystemTime::UNIX_EPOCH),
        }
    }

    fn names(v: &[FileEntry]) -> Vec<(String, bool, u64)> {
        v.iter()
            .map(|e| (e.name.clone(), e.is_dir, e.size))
            .collect()
    }

    #[test]
    fn inner_key_normalizes_separators() {
        assert_eq!(inner_key(Path::new("")), "");
        assert_eq!(inner_key(Path::new("a/b/")), "a/b");
        assert_eq!(inner_key(Path::new(r"a\b")), "a/b");
    }

    #[test]
    fn relative_to_requires_a_directory_boundary() {
        assert_eq!(relative_to("ab/c", "a"), None);
        assert_eq!(relative_to("a/c", "a"), Some("c"));
        assert_eq!(relative_to("a", "a"), None);
        assert_eq!(relative_to("x", ""), Some("x"));
    }

    #[test]
    fn root_lists_files_and_synthesized_dirs() {
        let items = [
            item("readme.txt", false, 5),
            item("src/main.rs", false, 9),
            item("src/lib/x.rs", false, 1),
            item(".hidden", false, 2),
        ];
        let v = direct_children(&items, Path::new("/a.rar"), Path::new(""));
        assert_eq!(
            names(&v),
            [
                ("readme.txt".into(), false, 5),
                ("src".into(), true, 0),
                (".hidden".into(), false, 2)
            ]
        );
        assert!(v[2].is_hidden);
        match &v[1].location {
            Location::Archive {
                archive_path,
                inner_path,
            } => {
                assert_eq!(**archive_path, Location::Local(PathBuf::from("/a.rar")));
                assert_eq!(inner_path, &PathBuf::from("src"));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn subdirectory_lists_only_its_children() {
        let items = [
            item("src", true, 0),
            item("src/main.rs", false, 9),
            item("src/lib", true, 0),
            item("src/lib/x.rs", false, 1),
            item("srcx/y", false, 1),
        ];
        let v = direct_children(&items, Path::new("a.7z"), Path::new("src"));
        assert_eq!(
            names(&v),
            [("main.rs".into(), false, 9), ("lib".into(), true, 0)]
        );
    }

    #[test]
    fn explicit_directory_record_keeps_its_time() {
        let t = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1000);
        let items = [
            item("d/f", false, 1),
            FlatItem {
                path: "d".into(),
                is_dir: true,
                size: 0,
                modified: Some(t),
            },
        ];
        let v = direct_children(&items, Path::new("a.zip"), Path::new(""));
        assert_eq!(v.len(), 1);
        assert!(v[0].is_dir);
        assert_eq!(v[0].modified, t);
    }
}
