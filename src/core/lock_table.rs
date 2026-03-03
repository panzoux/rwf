use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

pub struct LockTable {
    inner: Mutex<HashMap<String, Uuid>>,
}

impl LockTable {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(HashMap::new()),
        }
    }

    pub fn acquire(&self, path: &str, job_id: Uuid) -> Result<(), ()> {
        let mut table = self.inner.lock().unwrap();
        if table.contains_key(path) {
            return Err(());
        }
        table.insert(path.to_string(), job_id);
        Ok(())
    }

    pub fn release(&self, path: &str) {
        let mut table = self.inner.lock().unwrap();
        table.remove(path);
    }
}