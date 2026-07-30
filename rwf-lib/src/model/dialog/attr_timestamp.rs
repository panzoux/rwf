//! Attribute/timestamp change dialog content.
//!
//! Fetches each target's current attributes/timestamps via a synchronous
//! `metadata()` call at construction time — same precedent as `Dialog::file_info()`
//! (a cheap stat() syscall, not the kind of I/O that needs to go through a Job).
//! When multiple targets disagree on a field, that field starts in a "mixed"
//! state; only fields the user actually edits are applied across all targets
//! (see `to_change()` on both field types below).

use crate::model::Location;
use chrono::TimeZone;
use std::time::SystemTime;

const TIME_FORMAT: &str = "%Y-%m-%d %H:%M:%S";

/// A boolean attribute field that may start `Mixed` (`None`) across multiple targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriToggle {
    /// Value shared by every target when the dialog opened; `None` if they disagreed.
    initial: Option<bool>,
    /// Current UI value; starts equal to `initial`.
    pub current: Option<bool>,
}

impl TriToggle {
    fn new(initial: Option<bool>) -> Self {
        Self {
            initial,
            current: initial,
        }
    }

    pub fn toggle(&mut self) {
        self.current = Some(!self.current.unwrap_or(false));
    }

    /// `Some(value)` only if the user actually changed it from the opened state.
    pub fn to_change(self) -> Option<bool> {
        if self.current == self.initial {
            None
        } else {
            self.current
        }
    }

    pub fn label(self) -> &'static str {
        match self.current {
            Some(true) => "[X]",
            Some(false) => "[ ]",
            None => "[-]",
        }
    }
}

/// A text field seeded from the common value across targets (empty if mixed).
#[derive(Debug, Clone)]
pub struct AttrTextField {
    initial: String,
    pub text: String,
    pub cursor_pos: usize,
    pub scroll_pos: usize,
}

impl AttrTextField {
    fn new(initial: String) -> Self {
        let cursor_pos = initial.chars().count();
        Self {
            text: initial.clone(),
            initial,
            cursor_pos,
            scroll_pos: 0,
        }
    }

    /// `Some(text)` only if the user actually edited it away from the opened state.
    pub fn to_change(&self) -> Option<&str> {
        if self.text == self.initial {
            None
        } else {
            Some(&self.text)
        }
    }

    /// Stamp the current time into this field (the `Now` quick-action).
    pub fn set_now(&mut self) {
        self.text = format_time(SystemTime::now());
        self.cursor_pos = self.text.chars().count();
    }

    /// Overwrite the digit at the cursor in a fixed `"YYYY-MM-DD HH:MM:SS"`
    /// buffer and advance to the next digit position, skipping separators.
    /// If the field started empty (mixed across targets), it's first seeded
    /// with the current time so there's a skeleton to edit.
    ///
    /// `digit` must be `'0'..='9'` — callers are expected to have already
    /// filtered non-digit input (see `rwf-bin`'s dialog input handler),
    /// since typed characters that don't affect the field shouldn't be
    /// silently swallowed after appearing to be accepted.
    pub fn apply_timestamp_digit(&mut self, digit: char) {
        if self.text.is_empty() {
            self.set_now();
            self.cursor_pos = 0;
        }
        let pos = timestamp_pos::snap_to_digit(self.cursor_pos);
        let mut chars: Vec<char> = self.text.chars().collect();
        if pos < chars.len() {
            chars[pos] = digit;
            self.text = chars.into_iter().collect();
        }
        self.cursor_pos = timestamp_pos::next_digit_pos(pos);
    }

    pub fn move_timestamp_cursor_left(&mut self) {
        self.cursor_pos = timestamp_pos::prev_digit_pos(self.timestamp_cursor());
    }

    pub fn move_timestamp_cursor_right(&mut self) {
        self.cursor_pos = timestamp_pos::next_digit_pos(self.timestamp_cursor());
    }

    pub fn move_timestamp_cursor_home(&mut self) {
        self.cursor_pos = 0;
    }

    pub fn move_timestamp_cursor_end(&mut self) {
        self.cursor_pos = timestamp_pos::LAST_DIGIT;
    }

    fn timestamp_cursor(&self) -> usize {
        self.cursor_pos.min(timestamp_pos::LAST_DIGIT)
    }
}

/// Fixed-position helpers for editing a `"YYYY-MM-DD HH:MM:SS"` (19-char)
/// buffer as digit segments, skipping the `-`/` `/`:` separators at
/// indices 4, 7, 10, 13, 16.
mod timestamp_pos {
    const LEN: usize = 19;
    pub const LAST_DIGIT: usize = LEN - 1;

    fn is_separator(i: usize) -> bool {
        matches!(i, 4 | 7 | 10 | 13 | 16)
    }

    /// Snap a cursor position onto the nearest digit position at or after it.
    pub fn snap_to_digit(pos: usize) -> usize {
        let pos = pos.min(LAST_DIGIT);
        if is_separator(pos) {
            (pos + 1).min(LAST_DIGIT)
        } else {
            pos
        }
    }

    pub fn next_digit_pos(pos: usize) -> usize {
        let mut i = pos + 1;
        while i < LEN && is_separator(i) {
            i += 1;
        }
        i.min(LAST_DIGIT)
    }

    pub fn prev_digit_pos(pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        let mut i = pos - 1;
        while i > 0 && is_separator(i) {
            i -= 1;
        }
        i
    }
}

#[derive(Debug, Clone)]
pub struct AttrTimestampDialog {
    pub targets: Vec<Location>,

    #[cfg(windows)]
    pub readonly: TriToggle,
    #[cfg(windows)]
    pub hidden: TriToggle,
    #[cfg(windows)]
    pub system: TriToggle,
    #[cfg(windows)]
    pub archive: TriToggle,
    #[cfg(unix)]
    pub mode: AttrTextField,

    pub modified: AttrTextField,
    pub accessed: AttrTextField,
    /// Windows-only. Editable via `volume_info::set_windows_creation_time`.
    #[cfg(windows)]
    pub created: AttrTextField,

    /// Index into the platform's ordered focus list: fields..., OK, Cancel.
    pub focused_field: usize,
}

#[cfg(windows)]
impl AttrTimestampDialog {
    /// Number of focusable fields.
    pub const FIELD_COUNT: usize = 7; // readonly, hidden, system, archive, modified, accessed, created

    pub fn new(targets: Vec<Location>) -> Self {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_READONLY: u32 = 0x1;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        const FILE_ATTRIBUTE_SYSTEM: u32 = 0x4;
        const FILE_ATTRIBUTE_ARCHIVE: u32 = 0x20;

        let mut readonly_values = Vec::new();
        let mut hidden_values = Vec::new();
        let mut system_values = Vec::new();
        let mut archive_values = Vec::new();
        let mut modified_values = Vec::new();
        let mut accessed_values = Vec::new();
        let mut created_values = Vec::new();

        for target in &targets {
            if let Location::Local(path) = target {
                if let Ok(meta) = std::fs::metadata(path) {
                    let bits = meta.file_attributes();
                    readonly_values.push(bits & FILE_ATTRIBUTE_READONLY != 0);
                    hidden_values.push(bits & FILE_ATTRIBUTE_HIDDEN != 0);
                    system_values.push(bits & FILE_ATTRIBUTE_SYSTEM != 0);
                    archive_values.push(bits & FILE_ATTRIBUTE_ARCHIVE != 0);
                    if let Ok(m) = meta.modified() {
                        modified_values.push(m);
                    }
                    if let Ok(a) = meta.accessed() {
                        accessed_values.push(a);
                    }
                    if let Ok(c) = meta.created() {
                        created_values.push(c);
                    }
                }
            }
        }

        Self {
            targets,
            readonly: TriToggle::new(common_bool(&readonly_values)),
            hidden: TriToggle::new(common_bool(&hidden_values)),
            system: TriToggle::new(common_bool(&system_values)),
            archive: TriToggle::new(common_bool(&archive_values)),
            modified: AttrTextField::new(common_time_text(&modified_values)),
            accessed: AttrTextField::new(common_time_text(&accessed_values)),
            created: AttrTextField::new(common_time_text(&created_values)),
            focused_field: 0,
        }
    }

    pub fn to_attribute_change(&self) -> crate::model::AttributeChange {
        crate::model::AttributeChange {
            readonly: self.readonly.to_change(),
            hidden: self.hidden.to_change(),
            system: self.system.to_change(),
            archive: self.archive.to_change(),
        }
    }

    pub fn to_timestamp_change(&self) -> crate::model::TimestampChange {
        crate::model::TimestampChange {
            modified: self.modified.to_change().and_then(parse_time),
            accessed: self.accessed.to_change().and_then(parse_time),
            created: self.created.to_change().and_then(parse_time),
        }
    }
}

#[cfg(unix)]
impl AttrTimestampDialog {
    /// Number of focusable fields.
    pub const FIELD_COUNT: usize = 3; // mode, modified, accessed

    pub fn new(targets: Vec<Location>) -> Self {
        use std::os::unix::fs::PermissionsExt;

        let mut mode_values = Vec::new();
        let mut modified_values = Vec::new();
        let mut accessed_values = Vec::new();

        for target in &targets {
            if let Location::Local(path) = target {
                if let Ok(meta) = std::fs::metadata(path) {
                    mode_values.push(meta.permissions().mode() & 0o7777);
                    if let Ok(m) = meta.modified() {
                        modified_values.push(m);
                    }
                    if let Ok(a) = meta.accessed() {
                        accessed_values.push(a);
                    }
                }
            }
        }

        Self {
            targets,
            mode: AttrTextField::new(common_mode_text(&mode_values)),
            modified: AttrTextField::new(common_time_text(&modified_values)),
            accessed: AttrTextField::new(common_time_text(&accessed_values)),
            focused_field: 0,
        }
    }

    pub fn to_attribute_change(&self) -> crate::model::AttributeChange {
        crate::model::AttributeChange {
            mode: self
                .mode
                .to_change()
                .and_then(|s| u32::from_str_radix(s.trim(), 8).ok()),
        }
    }

    pub fn to_timestamp_change(&self) -> crate::model::TimestampChange {
        crate::model::TimestampChange {
            modified: self.modified.to_change().and_then(parse_time),
            accessed: self.accessed.to_change().and_then(parse_time),
        }
    }

    /// Live rwx preview of the currently-typed octal text (best-effort; blank
    /// if what's typed doesn't parse as octal yet).
    pub fn mode_rwx_preview(&self) -> String {
        match u32::from_str_radix(self.mode.text.trim(), 8) {
            Ok(mode) => format_rwx(mode),
            Err(_) => String::new(),
        }
    }
}

impl AttrTimestampDialog {
    pub fn ok_index(&self) -> usize {
        Self::FIELD_COUNT
    }

    pub fn cancel_index(&self) -> usize {
        Self::FIELD_COUNT + 1
    }
}

#[cfg(unix)]
fn format_rwx(mode: u32) -> String {
    let bit = |shift: u32, ch: char| -> char {
        if mode & (1 << shift) != 0 {
            ch
        } else {
            '-'
        }
    };
    [
        bit(8, 'r'),
        bit(7, 'w'),
        bit(6, 'x'),
        bit(5, 'r'),
        bit(4, 'w'),
        bit(3, 'x'),
        bit(2, 'r'),
        bit(1, 'w'),
        bit(0, 'x'),
    ]
    .iter()
    .collect()
}

#[cfg(windows)]
fn common_bool(values: &[bool]) -> Option<bool> {
    let first = *values.first()?;
    if values.iter().all(|v| *v == first) {
        Some(first)
    } else {
        None
    }
}

#[cfg(unix)]
fn common_mode_text(values: &[u32]) -> String {
    match values.first() {
        Some(first) if values.iter().all(|v| v == first) => format!("{:o}", first),
        _ => String::new(),
    }
}

fn common_time_text(values: &[SystemTime]) -> String {
    match values.first() {
        Some(first) if values.iter().all(|v| v == first) => format_time(*first),
        _ => String::new(),
    }
}

fn format_time(t: SystemTime) -> String {
    let datetime: chrono::DateTime<chrono::Local> = t.into();
    datetime.format(TIME_FORMAT).to_string()
}

fn parse_time(s: &str) -> Option<SystemTime> {
    let naive = chrono::NaiveDateTime::parse_from_str(s.trim(), TIME_FORMAT).ok()?;
    let local = chrono::Local.from_local_datetime(&naive).single()?;
    Some(local.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn tri_toggle_no_change_when_untouched() {
        let t = TriToggle::new(Some(false));
        assert_eq!(t.to_change(), None);
    }

    #[test]
    fn tri_toggle_reports_change_after_toggle() {
        let mut t = TriToggle::new(Some(false));
        t.toggle();
        assert_eq!(t.current, Some(true));
        assert_eq!(t.to_change(), Some(true));
    }

    #[test]
    fn tri_toggle_back_to_original_is_no_change() {
        let mut t = TriToggle::new(Some(false));
        t.toggle();
        t.toggle();
        assert_eq!(t.to_change(), None);
    }

    #[test]
    fn tri_toggle_mixed_starts_as_none_and_toggle_produces_a_concrete_value() {
        let mut t = TriToggle::new(None);
        assert_eq!(t.label(), "[-]");
        t.toggle();
        // Mixed -> toggled always yields Some(true) (current.unwrap_or(false) negated)
        assert_eq!(t.current, Some(true));
        assert_eq!(t.to_change(), Some(true));
    }

    #[test]
    fn text_field_no_change_when_untouched() {
        let f = AttrTextField::new("755".to_string());
        assert_eq!(f.to_change(), None);
    }

    #[test]
    fn text_field_reports_change_when_edited() {
        let mut f = AttrTextField::new("755".to_string());
        f.text = "644".to_string();
        assert_eq!(f.to_change(), Some("644"));
    }

    #[test]
    fn text_field_set_now_marks_touched() {
        let mut f = AttrTextField::new(String::new());
        f.set_now();
        assert!(f.to_change().is_some());
    }

    #[test]
    fn apply_timestamp_digit_overwrites_and_advances() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 0;
        f.apply_timestamp_digit('9');
        assert_eq!(&f.text, "9026-07-30 12:34:56");
        assert_eq!(f.cursor_pos, 1);
    }

    #[test]
    fn apply_timestamp_digit_advancing_past_separator_skips_it() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 3; // last digit of year
        f.apply_timestamp_digit('9');
        assert_eq!(&f.text, "2029-07-30 12:34:56");
        assert_eq!(f.cursor_pos, 5); // month tens digit, separator@4 skipped
    }

    #[test]
    fn apply_timestamp_digit_seeds_when_empty() {
        let mut f = AttrTextField::new(String::new());
        f.apply_timestamp_digit('5');
        assert_eq!(f.text.chars().count(), 19);
        assert_eq!(f.text.chars().next(), Some('5'));
    }

    #[test]
    fn apply_timestamp_digit_on_separator_snaps_forward_first() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 4; // the '-' separator
        f.apply_timestamp_digit('9');
        assert_eq!(&f.text, "2026-97-30 12:34:56"); // written into month tens digit
    }

    #[test]
    fn move_timestamp_cursor_left_skips_separator() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 5; // month tens digit
        f.move_timestamp_cursor_left();
        assert_eq!(f.cursor_pos, 3); // last year digit, separator@4 skipped
    }

    #[test]
    fn move_timestamp_cursor_right_skips_separator() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 3; // last year digit
        f.move_timestamp_cursor_right();
        assert_eq!(f.cursor_pos, 5); // month tens digit, separator@4 skipped
    }

    #[test]
    fn move_timestamp_cursor_left_at_start_stays_put() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 0;
        f.move_timestamp_cursor_left();
        assert_eq!(f.cursor_pos, 0);
    }

    #[test]
    fn move_timestamp_cursor_right_at_end_stays_put() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 18;
        f.move_timestamp_cursor_right();
        assert_eq!(f.cursor_pos, 18);
    }

    #[test]
    fn move_timestamp_cursor_home_and_end() {
        let mut f = AttrTextField::new("2026-07-30 12:34:56".to_string());
        f.cursor_pos = 9;
        f.move_timestamp_cursor_home();
        assert_eq!(f.cursor_pos, 0);
        f.move_timestamp_cursor_end();
        assert_eq!(f.cursor_pos, 18);
    }

    #[cfg(windows)]
    #[test]
    fn new_computes_common_value_for_uniform_targets() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        let file_b = temp_dir.path().join("b.txt");
        std::fs::write(&file_a, b"x").unwrap();
        std::fs::write(&file_b, b"y").unwrap();

        let dialog =
            AttrTimestampDialog::new(vec![Location::Local(file_a), Location::Local(file_b)]);

        // Neither file is hidden/readonly/system/archive by default
        assert_eq!(dialog.hidden.current, Some(false));
        assert_eq!(dialog.readonly.current, Some(false));
        // Both were just created, so modified/accessed should agree closely
        // enough in practice to be a concrete (non-empty) value most of the
        // time; assert only that the field isn't panicking to build.
        let _ = dialog.modified.text.clone();
    }

    #[cfg(windows)]
    #[test]
    fn new_marks_mixed_when_targets_disagree() {
        use std::os::windows::fs::MetadataExt;

        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        let file_b = temp_dir.path().join("b.txt");
        std::fs::write(&file_a, b"x").unwrap();
        std::fs::write(&file_b, b"y").unwrap();

        // Make file_b hidden so the two targets disagree on `hidden`.
        let attrs = std::fs::metadata(&file_b).unwrap().file_attributes();
        crate::volume_info::set_windows_file_attributes(&file_b, attrs | 0x2).unwrap();

        let dialog =
            AttrTimestampDialog::new(vec![Location::Local(file_a), Location::Local(file_b)]);

        assert_eq!(dialog.hidden.current, None); // mixed
        assert_eq!(dialog.hidden.label(), "[-]");
    }

    #[cfg(windows)]
    #[test]
    fn to_attribute_change_only_includes_touched_fields() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        std::fs::write(&file_a, b"x").unwrap();

        let mut dialog = AttrTimestampDialog::new(vec![Location::Local(file_a)]);
        dialog.hidden.toggle();

        let change = dialog.to_attribute_change();
        assert_eq!(change.hidden, Some(true));
        assert_eq!(change.readonly, None);
        assert_eq!(change.system, None);
        assert_eq!(change.archive, None);
    }

    #[cfg(unix)]
    #[test]
    fn mode_rwx_preview_reflects_typed_octal() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.sh");
        std::fs::write(&file_a, b"x").unwrap();

        let mut dialog = AttrTimestampDialog::new(vec![Location::Local(file_a)]);
        dialog.mode.text = "755".to_string();
        assert_eq!(dialog.mode_rwx_preview(), "rwxr-xr-x");
    }

    #[cfg(unix)]
    #[test]
    fn to_attribute_change_parses_octal_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.sh");
        std::fs::write(&file_a, b"x").unwrap();

        let mut dialog = AttrTimestampDialog::new(vec![Location::Local(file_a)]);
        dialog.mode.text = "755".to_string();

        let change = dialog.to_attribute_change();
        assert_eq!(change.mode, Some(0o755));
    }

    #[test]
    fn to_timestamp_change_parses_edited_field_only() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        std::fs::write(&file_a, b"x").unwrap();
        // Backdate so `set_now()` below is guaranteed to differ from the
        // dialog's initial (pre-formatted-to-the-second) value.
        let old_time = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_000_000_000);
        filetime::set_file_mtime(&file_a, filetime::FileTime::from_system_time(old_time)).unwrap();

        let mut dialog = AttrTimestampDialog::new(vec![Location::Local(file_a)]);
        dialog.modified.set_now();

        let change = dialog.to_timestamp_change();
        assert!(change.modified.is_some());
        assert_eq!(change.accessed, None);
    }

    #[cfg(windows)]
    #[test]
    fn to_timestamp_change_includes_created_when_touched() {
        let temp_dir = TempDir::new().unwrap();
        let file_a = temp_dir.path().join("a.txt");
        std::fs::write(&file_a, b"x").unwrap();

        let mut dialog = AttrTimestampDialog::new(vec![Location::Local(file_a)]);
        assert_eq!(dialog.created.to_change(), None); // untouched by default
                                                      // Set explicitly rather than via `set_now()`: the file's real
                                                      // creation time (just stamped by `std::fs::write` above) and "now" a
                                                      // moment later can land in the same second once formatted, which
                                                      // would make this a same-second-collision flake instead of a real
                                                      // touched-detection test.
        dialog.created.text = "2020-01-01 00:00:00".to_string();

        let change = dialog.to_timestamp_change();
        assert!(change.created.is_some());
    }

    #[test]
    fn format_and_parse_time_roundtrip() {
        let now = SystemTime::now();
        let text = format_time(now);
        let parsed = parse_time(&text).unwrap();
        // Sub-second precision is lost by the "%Y-%m-%d %H:%M:%S" format.
        let diff = now
            .duration_since(parsed)
            .or_else(|_| parsed.duration_since(now))
            .unwrap();
        assert!(diff.as_secs() < 1);
    }
}
