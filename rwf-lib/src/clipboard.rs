//! Cross-platform clipboard writes.
//!
//! Two backends, because neither covers every situation on its own:
//!
//! * **Native** ([`arboard`]) — Win32 `CF_UNICODETEXT`, macOS `NSPasteboard`, X11,
//!   Wayland. In-process, so there is no `clip.exe` console flash, no trailing
//!   newline from `echo`, and no shell quoting to get wrong. Fails when there is no
//!   display server: a bare SSH session, a Linux TTY, a container.
//! * **OSC 52** — an escape sequence the *terminal emulator* interprets, so the text
//!   lands on the clipboard of whichever machine the human is sitting at. This is what
//!   makes copying work over SSH. Not all terminals implement it, and many cap the
//!   payload size, so it is the fallback rather than the default.
//!
//! `Backend::Auto` tries native and falls back to OSC 52.
//!
//! # X11 caveat
//!
//! X11 has no clipboard daemon: the selection is owned by a *live process*, and the
//! content disappears when that process exits. So the [`arboard::Clipboard`] is
//! created once and kept for the lifetime of the app rather than per copy — otherwise
//! the text would be gone before the user could paste it. It still does not survive
//! rwf exiting. That is X11's design, not something this module can fix.

use std::sync::{Mutex, OnceLock};

/// Which mechanism [`set_text`] should use. Configured by `Clipboard.Backend`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum ClipboardBackend {
    /// Native first, OSC 52 if that fails. The right choice almost always.
    #[default]
    Auto,
    /// Native only — fail rather than emit an escape sequence.
    Native,
    /// OSC 52 only. Force this when rwf runs on a remote host and you want the text
    /// on your *local* clipboard even though the remote has a display server.
    Osc52,
    /// Copying disabled.
    None,
}

/// The long-lived native clipboard handle. See the X11 caveat in the module docs.
fn native_handle() -> Option<&'static Mutex<arboard::Clipboard>> {
    static HANDLE: OnceLock<Option<Mutex<arboard::Clipboard>>> = OnceLock::new();
    HANDLE
        .get_or_init(|| match arboard::Clipboard::new() {
            Ok(c) => Some(Mutex::new(c)),
            Err(e) => {
                tracing::debug!("native clipboard unavailable: {e}");
                None
            }
        })
        .as_ref()
}

fn set_native(text: &str) -> Result<(), String> {
    let handle = native_handle().ok_or_else(|| "no native clipboard available".to_string())?;
    let mut guard = handle
        .lock()
        .map_err(|_| "clipboard handle poisoned".to_string())?;
    guard.set_text(text.to_string()).map_err(|e| e.to_string())
}

/// Write `text` to the clipboard using `backend`.
///
/// Returns the name of the backend that succeeded, for the task-panel status line.
pub fn set_text(text: &str, backend: ClipboardBackend) -> Result<&'static str, String> {
    match backend {
        ClipboardBackend::None => Err("clipboard is disabled (Clipboard.Backend = None)".into()),
        ClipboardBackend::Native => set_native(text).map(|()| "native"),
        ClipboardBackend::Osc52 => write_osc52(text).map(|()| "OSC 52"),
        ClipboardBackend::Auto => match set_native(text) {
            Ok(()) => Ok("native"),
            Err(native_err) => match write_osc52(text) {
                Ok(()) => Ok("OSC 52"),
                // Report both, or the user sees only the fallback's failure and has no
                // idea the native path was even attempted.
                Err(osc_err) => Err(format!("native: {native_err}; OSC 52: {osc_err}")),
            },
        },
    }
}

/// Largest payload we will emit as OSC 52.
///
/// Terminals cap what they accept and simply drop anything longer, so a silent
/// truncation would be worse than an error. 100 KB of base64 is far beyond any
/// plausible path list and below the limits of terminals that support OSC 52 at all.
const OSC52_MAX_BASE64: usize = 100_000;

/// Build the OSC 52 sequence for `text`, wrapping it for tmux/screen when needed.
///
/// Kept separate from the write so it can be unit-tested without a terminal.
pub fn osc52_sequence(text: &str, multiplexer: Multiplexer) -> Result<String, String> {
    let payload = base64_encode(text.as_bytes());
    if payload.len() > OSC52_MAX_BASE64 {
        return Err(format!(
            "too large for OSC 52 ({} bytes encoded; limit {OSC52_MAX_BASE64})",
            payload.len()
        ));
    }
    // "c" selects the CLIPBOARD selection (as opposed to primary).
    let inner = format!("\x1b]52;c;{payload}\x07");
    Ok(match multiplexer {
        Multiplexer::None => inner,
        // tmux only forwards escape sequences to the outer terminal inside a DCS
        // passthrough, and each ESC in the payload must be doubled.
        Multiplexer::Tmux => format!("\x1bPtmux;{}\x1b\\", inner.replace('\x1b', "\x1b\x1b")),
        // screen's passthrough has a ~768-byte chunk limit; strings this short are fine.
        Multiplexer::Screen => format!("\x1bP{inner}\x1b\\"),
    })
}

/// Terminal multiplexer wrapping required around an OSC 52 sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Multiplexer {
    None,
    Tmux,
    Screen,
}

impl Multiplexer {
    /// Detect the multiplexer from the environment.
    pub fn detect() -> Self {
        if std::env::var_os("TMUX").is_some() {
            return Self::Tmux;
        }
        match std::env::var("TERM") {
            Ok(term) if term.starts_with("screen") => Self::Screen,
            _ => Self::None,
        }
    }
}

fn write_osc52(text: &str) -> Result<(), String> {
    use std::io::Write;
    let seq = osc52_sequence(text, Multiplexer::detect())?;
    // Written to the same stdout the TUI already renders through; the terminal
    // consumes the sequence rather than displaying it.
    let mut out = std::io::stdout().lock();
    out.write_all(seq.as_bytes())
        .and_then(|()| out.flush())
        .map_err(|e| e.to_string())
}

/// Standard base64 (RFC 4648 §4) with padding.
///
/// Hand-rolled to avoid a dependency for ~15 lines; OSC 52 is the only caller.
fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        let idx = [n >> 18 & 63, n >> 12 & 63, n >> 6 & 63, n & 63];
        out.push(TABLE[idx[0] as usize] as char);
        out.push(TABLE[idx[1] as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[idx[2] as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[idx[3] as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_test_vectors() {
        // RFC 4648 §10 — these also exercise both padding lengths.
        for (input, expected) in [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foob", "Zm9vYg=="),
            ("fooba", "Zm9vYmE="),
            ("foobar", "Zm9vYmFy"),
        ] {
            assert_eq!(base64_encode(input.as_bytes()), expected, "input {input:?}");
        }
    }

    #[test]
    fn base64_handles_non_ascii_and_high_bytes() {
        assert_eq!(base64_encode("日本".as_bytes()), "5pel5pys");
        assert_eq!(base64_encode(&[0xff, 0xff, 0xff]), "////");
    }

    #[test]
    fn osc52_sequence_has_the_expected_shape() {
        let seq = osc52_sequence("foo", Multiplexer::None).expect("short payload");
        assert_eq!(seq, "\x1b]52;c;Zm9v\x07");
    }

    #[test]
    fn osc52_is_wrapped_for_tmux_with_escapes_doubled() {
        let seq = osc52_sequence("foo", Multiplexer::Tmux).expect("short payload");
        // tmux passthrough: DCS intro, doubled ESC in the payload, ST terminator.
        assert_eq!(seq, "\x1bPtmux;\x1b\x1b]52;c;Zm9v\x07\x1b\\");
    }

    #[test]
    fn osc52_is_wrapped_for_screen() {
        let seq = osc52_sequence("foo", Multiplexer::Screen).expect("short payload");
        assert_eq!(seq, "\x1bP\x1b]52;c;Zm9v\x07\x1b\\");
    }

    #[test]
    fn osc52_refuses_an_oversized_payload_rather_than_truncating() {
        // Terminals silently drop over-long sequences; a partial clipboard would be
        // worse than a visible error.
        let huge = "x".repeat(OSC52_MAX_BASE64);
        let err = osc52_sequence(&huge, Multiplexer::None).expect_err("should refuse");
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn disabled_backend_reports_why() {
        let err = set_text("anything", ClipboardBackend::None).expect_err("None never copies");
        assert!(err.contains("disabled"), "{err}");
    }

    #[test]
    fn backend_default_is_auto() {
        assert_eq!(ClipboardBackend::default(), ClipboardBackend::Auto);
    }

    /// A genuine round-trip through the OS clipboard.
    ///
    /// Skipped on CI, which is headless and has no clipboard at all — same reasoning
    /// (and the same `is_ci()` gate) as the real-Recycle-Bin trash tests. Every other
    /// test in this module deliberately asserts on the *bytes we would emit* rather
    /// than on an OS side effect, so this is the only one that needs the gate.
    #[test]
    fn native_round_trip_preserves_text_exactly() {
        if crate::test_utils::is_ci() {
            eprintln!("skipping: no clipboard on CI");
            return;
        }
        let Some(handle) = native_handle() else {
            eprintln!("skipping: no native clipboard in this session");
            return;
        };

        // Multi-line, no trailing newline, a space in a name, and non-ASCII — the exact
        // shape `$MPL` produces, and everything `echo … | clip.exe` would have corrupted.
        let sent = "C:\\dir\\a.txt\nC:\\dir\\b c.txt\n日本語";
        set_text(sent, ClipboardBackend::Native).expect("native copy");

        let got = handle
            .lock()
            .expect("clipboard handle")
            .get_text()
            .expect("read back");
        assert_eq!(got, sent, "clipboard text was altered in transit");
    }
}
