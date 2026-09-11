//! What kind of failure a job hit — decided from the error's structure, never from
//! the wording of its message (Phase 7.22 §3.2).
//!
//! The failure dialog used to pick its title by substring-matching the error text
//! (`contains("not found")`). The OS localizes that text: on a Japanese Windows an
//! unreachable share reports `ネットワーク名が見つかりません。 (os error 67)`, no arm
//! matched, and every failure there was titled "Operation Failed".
//!
//! Two structured sources are used instead:
//!
//! - an `io::Error` in hand: its raw OS code, then its `io::ErrorKind`
//!   ([`FailureKind::from_io_error`]);
//! - a job's failure string: the `(os error N)` suffix, which std appends to every
//!   OS-originated `io::Error` it formats and which is **not** localized — the code is
//!   recovered and classified exactly as above ([`FailureKind::classify`]).
//!
//! Only a message with no OS code falls back to English words, and those messages
//! are rwf's own strings or std's kind descriptions ("entity not found"), neither of
//! which is translated.
//!
//! Carrying a kind through `OpResult::Failed` itself was considered and not done: it
//! is constructed at ~80 sites (39 in the executor alone) as a plain `String`, and
//! the OS code survives every one of them verbatim inside `format!("{e:#}")`.

use std::io;

/// The failure classes a user can act on differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureKind {
    /// The path does not exist (any more).
    NotFound,
    /// It exists, and this account may not use it.
    PermissionDenied,
    /// A network share, host or the network itself cannot be reached.
    NetworkUnavailable,
    /// Something answered too slowly.
    TimedOut,
    /// The path is malformed or does not name what the operation needs.
    InvalidPath,
    /// Anything else — the message is still shown verbatim.
    Other,
}

impl FailureKind {
    /// Classify an error that is still an `io::Error`.
    pub fn from_io_error(error: &io::Error) -> Self {
        if let Some(kind) = error.raw_os_error().and_then(network_os_code) {
            return kind;
        }
        Self::from_error_kind(error.kind())
    }

    /// Classify a job's failure message.
    pub fn classify(message: &str) -> Self {
        if let Some(code) = os_error_code(message) {
            return Self::from_io_error(&io::Error::from_raw_os_error(code));
        }
        // No OS code: an rwf-authored message or a kind-only std error. Neither is
        // localized, so words are a sound signal here — and only here.
        let lower = message.to_lowercase();
        if lower.contains("permission") || lower.contains("access denied") {
            Self::PermissionDenied
        } else if lower.contains("not found")
            || lower.contains("does not exist")
            || lower.contains("no such file")
        {
            Self::NotFound
        } else if lower.contains("timed out") {
            Self::TimedOut
        } else if lower.contains("invalid") {
            Self::InvalidPath
        } else {
            Self::Other
        }
    }

    fn from_error_kind(kind: io::ErrorKind) -> Self {
        use io::ErrorKind as K;
        match kind {
            K::NotFound => Self::NotFound,
            K::PermissionDenied => Self::PermissionDenied,
            K::TimedOut => Self::TimedOut,
            K::HostUnreachable
            | K::NetworkUnreachable
            | K::NetworkDown
            | K::NotConnected
            | K::ConnectionRefused
            | K::ConnectionReset
            | K::ConnectionAborted
            | K::StaleNetworkFileHandle => Self::NetworkUnavailable,
            K::InvalidInput | K::InvalidFilename | K::NotADirectory => Self::InvalidPath,
            _ => Self::Other,
        }
    }

    /// Title for a generic failure dialog. The first four keep the wording the
    /// dialog has always used.
    pub fn title(self) -> &'static str {
        match self {
            Self::NotFound => "File Not Found",
            Self::PermissionDenied => "Permission Denied",
            Self::InvalidPath => "Invalid Path",
            Self::Other => "Operation Failed",
            Self::NetworkUnavailable => "Network Location Unavailable",
            Self::TimedOut => "Timed Out",
        }
    }

    /// Title for a directory that could not be read — the pane-read dialog.
    pub fn read_title(self) -> &'static str {
        match self {
            Self::NotFound => "Directory Not Found",
            Self::PermissionDenied => "Permission Denied",
            Self::NetworkUnavailable => "Directory Unavailable",
            Self::TimedOut => "Directory Read Timed Out",
            Self::InvalidPath => "Not a Readable Directory",
            Self::Other => "Directory Read Failed",
        }
    }

    /// What the user can conclude or try, when there is anything useful to say.
    pub fn hint(self) -> Option<&'static str> {
        match self {
            Self::NotFound => Some("It may have been moved, renamed or deleted."),
            Self::PermissionDenied => Some("This account is not allowed to open it."),
            Self::NetworkUnavailable => Some("The share may be offline, or the host unreachable."),
            Self::TimedOut => Some("The host did not answer in time; it may be slow or offline."),
            Self::InvalidPath => Some("The path does not name a directory that can be read."),
            Self::Other => None,
        }
    }
}

/// The code from std's `(os error N)` suffix — the last one, since a path quoted
/// earlier in the message could contain the same text.
fn os_error_code(message: &str) -> Option<i32> {
    let (_, tail) = message.rsplit_once("(os error ")?;
    let (code, _) = tail.split_once(')')?;
    code.trim().parse().ok()
}

/// Network failures std leaves `Uncategorized`. Windows' SMB errors are the ones that
/// matter in practice — the dead-share report was `os error 67`.
fn network_os_code(code: i32) -> Option<FailureKind> {
    #[cfg(windows)]
    {
        const ERROR_REM_NOT_LIST: i32 = 51;
        const ERROR_BAD_NETPATH: i32 = 53;
        const ERROR_NETWORK_BUSY: i32 = 54;
        const ERROR_UNEXP_NET_ERR: i32 = 59;
        const ERROR_NETNAME_DELETED: i32 = 64;
        const ERROR_BAD_NET_NAME: i32 = 67;
        const ERROR_NO_NET_OR_BAD_PATH: i32 = 1203;
        const ERROR_NO_NETWORK: i32 = 1222;
        const ERROR_NOT_CONNECTED: i32 = 2250;
        match code {
            ERROR_REM_NOT_LIST
            | ERROR_BAD_NETPATH
            | ERROR_NETWORK_BUSY
            | ERROR_UNEXP_NET_ERR
            | ERROR_NETNAME_DELETED
            | ERROR_BAD_NET_NAME
            | ERROR_NO_NET_OR_BAD_PATH
            | ERROR_NO_NETWORK
            | ERROR_NOT_CONNECTED => Some(FailureKind::NetworkUnavailable),
            _ => None,
        }
    }
    #[cfg(target_os = "linux")]
    {
        const EHOSTDOWN: i32 = 112;
        (code == EHOSTDOWN).then_some(FailureKind::NetworkUnavailable)
    }
    #[cfg(target_os = "macos")]
    {
        const EHOSTDOWN: i32 = 64;
        (code == EHOSTDOWN).then_some(FailureKind::NetworkUnavailable)
    }
    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        let _ = code;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole approach rests on std's formatting of OS errors. If a future std
    /// drops the suffix, this fails before any dialog silently regresses.
    #[test]
    fn std_still_formats_os_errors_with_their_code() {
        let text = io::Error::from_raw_os_error(2).to_string();
        assert!(
            text.ends_with("(os error 2)"),
            "std formatting changed: {text}"
        );
    }

    #[test]
    fn a_code_is_read_back_through_an_anyhow_context_chain() {
        let inner = io::Error::from_raw_os_error(2);
        let err = anyhow::Error::new(inner).context("Failed to read directory C:\\x");
        assert_eq!(os_error_code(&format!("{err:#}")), Some(2));
    }

    #[test]
    fn the_last_code_wins_over_one_quoted_in_a_path() {
        assert_eq!(
            os_error_code("Failed to read directory C:\\odd (os error 5) name: gone (os error 2)"),
            Some(2)
        );
    }

    /// The reported bug: a Japanese Windows message matched nothing and every failure
    /// became "Operation Failed".
    #[cfg(windows)]
    #[test]
    fn a_localized_windows_network_error_is_classified_by_its_code() {
        let message = "Failed to read directory \\\\192.168.11.24\\testfol: \
                       ネットワーク名が見つかりません。 (os error 67)";
        assert_eq!(
            FailureKind::classify(message),
            FailureKind::NetworkUnavailable
        );
    }

    #[cfg(windows)]
    #[test]
    fn localized_not_found_and_access_denied_are_classified_by_code() {
        assert_eq!(
            FailureKind::classify("指定されたパスが見つかりません。 (os error 3)"),
            FailureKind::NotFound
        );
        assert_eq!(
            FailureKind::classify("アクセスが拒否されました。 (os error 5)"),
            FailureKind::PermissionDenied
        );
    }

    #[cfg(unix)]
    #[test]
    fn unix_errno_codes_are_classified() {
        assert_eq!(
            FailureKind::classify("No such file or directory (os error 2)"),
            FailureKind::NotFound
        );
        assert_eq!(
            FailureKind::classify("Permission denied (os error 13)"),
            FailureKind::PermissionDenied
        );
    }

    #[test]
    fn an_io_error_kind_without_a_code_is_classified() {
        let err = io::Error::new(io::ErrorKind::TimedOut, "slow");
        assert_eq!(FailureKind::from_io_error(&err), FailureKind::TimedOut);
    }

    #[test]
    fn english_messages_without_a_code_still_classify() {
        assert_eq!(
            FailureKind::classify("Permission denied"),
            FailureKind::PermissionDenied
        );
        assert_eq!(
            FailureKind::classify("File not found"),
            FailureKind::NotFound
        );
        assert_eq!(
            FailureKind::classify("Invalid path"),
            FailureKind::InvalidPath
        );
        assert_eq!(FailureKind::classify("Disk full"), FailureKind::Other);
    }

    /// A code that means something else must not be overridden by a word in the text.
    #[test]
    fn a_code_outranks_the_words_around_it() {
        // "not found" in the prose, but code 5/13 is access denied.
        #[cfg(windows)]
        let message = "config not found somewhere (os error 5)";
        #[cfg(unix)]
        let message = "config not found somewhere (os error 13)";
        assert_eq!(
            FailureKind::classify(message),
            FailureKind::PermissionDenied
        );
    }
}
