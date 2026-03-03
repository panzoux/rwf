#[derive(Debug, Clone)]
pub enum FailureReason {
    IoError(String),
    DependencyFailed,
    ForcedTermination,
}