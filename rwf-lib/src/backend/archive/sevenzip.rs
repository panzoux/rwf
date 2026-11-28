//! Archive access through 7-Zip's shared library (`7z.dll` / `7z.so`, plan/8.11).
//!
//! The library is resolved lazily: the first archive operation that needs it loads
//! it on the worker thread running that job, never on the input thread (see
//! docs/IMPLICIT_CONTRACTS.md §8). The result — library or "not available" — is
//! cached until the configured path changes.

use super::listing::{direct_children, inner_key, relative_to, FlatItem};
use crate::model::FileEntry;
use rwf_7zlib::{file_sink, safe_join, Library, OpResult};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// The 7-Zip library, resolved on first use.
#[derive(Debug, Default)]
pub struct SevenZip {
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    /// `Archive.SevenZipLibrary` from config.json; `None` searches the usual places.
    configured: Option<PathBuf>,
    /// `None`: not tried yet. `Some(None)`: tried, not available.
    resolved: Option<Option<Arc<Library>>>,
}

impl SevenZip {
    pub fn new(configured: Option<PathBuf>) -> Self {
        Self {
            state: Mutex::new(State {
                configured,
                resolved: None,
            }),
        }
    }

    /// Use a different configured path from the next operation on. Unchanged paths
    /// keep the loaded library.
    pub fn set_configured(&self, configured: Option<PathBuf>) {
        if let Ok(mut s) = self.state.lock() {
            if s.configured != configured {
                s.configured = configured;
                s.resolved = None;
            }
        }
    }

    /// The library, loading it if this is the first call. Blocking: worker only.
    pub fn library(&self) -> Option<Arc<Library>> {
        let mut s = self.state.lock().ok()?;
        if s.resolved.is_none() {
            let found = match Library::find(s.configured.as_deref()) {
                Ok(lib) => Some(Arc::new(lib)),
                Err(e) => {
                    tracing::info!("7-Zip not used: {e}");
                    None
                }
            };
            s.resolved = Some(found);
        }
        s.resolved.clone().flatten()
    }
}

/// Why a 7-Zip operation did not complete.
#[derive(Debug)]
pub enum SzError {
    /// 7-Zip has no format that accepts the file: try the built-in handler.
    NotOpened,
    /// Every failing item used a compression method this 7-Zip build lacks
    /// (RAR in Homebrew's / Debian's 7z.so): try another RAR back end.
    UnsupportedMethod,
    /// A real error (I/O, damaged data, cancellation, password …).
    Failed(anyhow::Error),
}

impl From<rwf_7zlib::Error> for SzError {
    fn from(e: rwf_7zlib::Error) -> Self {
        match e {
            rwf_7zlib::Error::CannotOpen(_) => SzError::NotOpened,
            rwf_7zlib::Error::Cancelled => SzError::Failed(anyhow::anyhow!("Operation cancelled")),
            rwf_7zlib::Error::Io(e) => SzError::Failed(e.into()),
            other => SzError::Failed(anyhow::anyhow!("7-Zip: {other}")),
        }
    }
}

impl From<std::io::Error> for SzError {
    fn from(e: std::io::Error) -> Self {
        SzError::Failed(e.into())
    }
}

fn flat_items(a: &rwf_7zlib::Archive<'_>) -> Vec<FlatItem> {
    a.items()
        .iter()
        .map(|i| FlatItem {
            path: i.path.clone(),
            is_dir: i.is_dir,
            size: i.size,
            modified: i.modified,
        })
        .collect()
}

/// The direct children of `inner` in the archive.
pub fn list(
    lib: &Library,
    archive: &Path,
    inner: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<FileEntry>, SzError> {
    if cancel.is_cancelled() {
        return Err(SzError::Failed(anyhow::anyhow!("Operation cancelled")));
    }
    let a = lib.open(archive)?;
    Ok(direct_children(&flat_items(&a), archive, inner))
}

/// Extract the single file at `inner` to the file path `dest`.
pub fn extract_file(
    lib: &Library,
    archive: &Path,
    inner: &Path,
    dest: &Path,
    cancel: &CancellationToken,
) -> Result<(), SzError> {
    let a = lib.open(archive)?;
    let key = inner_key(inner);
    let item = a
        .items()
        .iter()
        .find(|i| !i.is_dir && i.path == key)
        .ok_or_else(|| SzError::Failed(anyhow::anyhow!("File not found in archive: {key}")))?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let plan = vec![(item.index, Some(dest.to_path_buf()), false)];
    run(&a, plan, cancel)
}

/// Extract everything below `inner` (the whole archive for `None` or the root)
/// into the directory `dest`, keeping the relative layout.
pub fn extract_tree(
    lib: &Library,
    archive: &Path,
    inner: Option<&Path>,
    dest: &Path,
    cancel: &CancellationToken,
) -> Result<(), SzError> {
    let a = lib.open(archive)?;
    let key = inner.map(inner_key).unwrap_or_default();
    std::fs::create_dir_all(dest)?;
    let mut plan = Vec::new();
    for item in a.items() {
        let Some(rel) = relative_to(&item.path, &key) else {
            continue;
        };
        let target = safe_join(dest, rel);
        if target.is_none() {
            tracing::warn!(
                "skipping archive entry outside the destination: {}",
                item.path
            );
        }
        plan.push((item.index, target, item.is_dir));
    }
    if plan.is_empty() {
        return Ok(());
    }
    run(&a, plan, cancel)
}

/// Extract `(index, destination, is_dir)` triples. Directories are created and get
/// no data; `None` destinations are skipped. Outputs of items that failed are removed.
fn run(
    a: &rwf_7zlib::Archive<'_>,
    plan: Vec<(u32, Option<PathBuf>, bool)>,
    cancel: &CancellationToken,
) -> Result<(), SzError> {
    let indices: Vec<u32> = plan.iter().map(|(i, _, _)| *i).collect();
    let lookup: std::collections::HashMap<u32, (Option<PathBuf>, bool)> =
        plan.into_iter().map(|(i, p, d)| (i, (p, d))).collect();
    let mut target = |index: u32| -> std::io::Result<Option<Box<dyn std::io::Write + Send>>> {
        match lookup.get(&index) {
            Some((Some(path), true)) => {
                std::fs::create_dir_all(path)?;
                Ok(None)
            }
            Some((Some(path), false)) => {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                Ok(Some(file_sink(std::fs::File::create(path)?)))
            }
            _ => Ok(None),
        }
    };
    let results = a.extract(&indices, &mut target, &|| cancel.is_cancelled())?;

    let failed: Vec<(u32, OpResult)> = results
        .into_iter()
        .filter(|(_, r)| *r != OpResult::Ok)
        .collect();
    if failed.is_empty() {
        return Ok(());
    }
    for (index, _) in &failed {
        if let Some((Some(path), false)) = lookup.get(index) {
            let _ = std::fs::remove_file(path);
        }
    }
    if failed
        .iter()
        .all(|(_, r)| *r == OpResult::UnsupportedMethod)
    {
        return Err(SzError::UnsupportedMethod);
    }
    let (index, result) = failed[0];
    let name = a
        .items()
        .get(index as usize)
        .map(|i| i.path.as_str())
        .unwrap_or("?");
    let more = if failed.len() > 1 {
        format!(" (and {} more)", failed.len() - 1)
    } else {
        String::new()
    };
    Err(SzError::Failed(anyhow::anyhow!(
        "7-Zip could not extract {name}: {result}{more}"
    )))
}
