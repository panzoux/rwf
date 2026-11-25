//! Job identity lives in `model` so that pane/tab state can name a job
//! without depending on the `job` module (which depends on `model`).

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a job
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct JobId(pub Uuid);

impl JobId {
    /// Generate a new unique job ID
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for JobId {
    fn default() -> Self {
        Self::new()
    }
}
