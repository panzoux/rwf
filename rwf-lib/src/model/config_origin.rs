//! Where a config value came from (moved down from `config_layers`).

/// Where a custom function's effective definition came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigOrigin {
    BuiltIn,
    User,
}

impl ConfigOrigin {
    pub fn label(self) -> &'static str {
        match self {
            Self::BuiltIn => "built-in",
            Self::User => "user",
        }
    }
}
