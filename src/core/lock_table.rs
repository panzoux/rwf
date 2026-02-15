use std::collections::HashSet;
use std::path::PathBuf;

pub struct LockTable {
    locked_paths: HashSet<PathBuf>,
}

impl LockTable {
    pub fn new() -> Self {
        Self {
            locked_paths: HashSet::new(),
        }
    }

    pub fn try_lock(&mut self, path: PathBuf) -> bool {
        if self.locked_paths.contains(&path) {
            return false;
        }
        self.locked_paths.insert(path);
        true
    }

    pub fn unlock(&mut self, path: &PathBuf) {
        self.locked_paths.remove(path);
    }
}
