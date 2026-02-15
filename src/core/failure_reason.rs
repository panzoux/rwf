#[derive(Debug)]
pub enum FailureReason {
    IoError(std::io::Error),
    DependencyFailed,
    DependencyCancelled,
    LockConflict,
    ForceTerminated,
    Unknown,
}
