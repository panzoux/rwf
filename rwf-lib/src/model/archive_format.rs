//! Archive formats (moved down from `input` so `config`/`dialog` need not depend on it).

use serde::{Deserialize, Serialize};

/// Archive format for compression operations
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ArchiveFormat {
    #[default]
    ZIP,
    SevenZip,
    Tar,
    TarGz,
}
