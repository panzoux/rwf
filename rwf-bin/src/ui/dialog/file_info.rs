//! File information dialog rendering.
//!
//! Split from dialog/mod.rs in M3 (move-only; snapshot-protected).

use ratatui::{layout::Rect, widgets::Paragraph, Frame};
use unicode_width::UnicodeWidthChar;

use crate::ui::{sanitize_for_display, smart_truncate, truncate_to_width};

/// Wrap already-decoded, already-sanitized display text into up to
/// `max_lines` rows of at most `max_width` display columns each — width-aware
/// (a CJK character counts as 2 columns), not char-count-aware, so CJK text
/// wraps at half the column budget of ASCII text. If the text needs more
/// than `max_lines` rows, the last row is truncated with an ellipsis.
fn wrap_decoded_text(text: &str, max_width: usize, max_lines: usize) -> Vec<String> {
    if max_width == 0 || max_lines == 0 {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_width = 0usize;

    for ch in text.chars() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1);
        if current_width + ch_width > max_width {
            lines.push(std::mem::take(&mut current));
            current_width = 0;
            if lines.len() == max_lines {
                // More text remains than the row budget allows — mark the
                // last visible row as truncated rather than growing further.
                // The last row is already exactly `max_width` columns wide
                // (that's why it was flushed), so `truncate_to_width` alone
                // is a no-op here (it only shortens strings that overflow
                // the target width) — shave one display column off first to
                // make room for the ellipsis glyph.
                if let Some(last) = lines.last_mut() {
                    let shortened = truncate_to_width(last, max_width.saturating_sub(1), "");
                    *last = format!("{shortened}…");
                }
                return lines;
            }
        }
        current.push(ch);
        current_width += ch_width;
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

fn fmt_size(bytes: u64) -> String {
    const KB: u64 = 1_024;
    const MB: u64 = 1_024 * KB;
    const GB: u64 = 1_024 * MB;
    if bytes >= GB {
        format!("{:.2} GB ({} bytes)", bytes as f64 / GB as f64, bytes)
    } else if bytes >= MB {
        format!("{:.2} MB ({} bytes)", bytes as f64 / MB as f64, bytes)
    } else if bytes >= KB {
        format!("{:.1} KB ({} bytes)", bytes as f64 / KB as f64, bytes)
    } else {
        format!("{} bytes", bytes)
    }
}

fn fmt_time(t: Option<std::time::SystemTime>) -> String {
    match t {
        None => "N/A".to_string(),
        Some(st) => {
            let dt: chrono::DateTime<chrono::Local> = st.into();
            dt.format("%Y-%m-%d %H:%M:%S").to_string()
        }
    }
}

#[allow(unused_variables, unused_mut)]
#[allow(clippy::too_many_arguments)]
pub(super) fn render_file_info_dialog(
    frame: &mut Frame,
    area: Rect,
    file_name: &str,
    file_path: &str,
    size: u64,
    created: Option<std::time::SystemTime>,
    modified: std::time::SystemTime,
    accessed: Option<std::time::SystemTime>,
    is_dir: bool,
    is_readonly: bool,
    #[cfg(unix)] permissions: Option<u32>,
    #[cfg(unix)] owner: Option<&str>,
    #[cfg(unix)] group: Option<&str>,
    link_target: Option<&str>,
    link_kind: Option<&rwf_lib::model::LinkKind>,
    detecting: bool,
    detected_type: Option<&str>,
    header_bytes: Option<&[u8]>,
    header_hex_mode: bool,
) {
    let base = crate::ui::dialog::common::DIALOG_TEXT;
    let label = crate::ui::dialog::common::DIALOG_DIM;
    let hint = crate::ui::dialog::common::DIALOG_DIM;
    let w = area.width.saturating_sub(4) as usize;

    let type_label = match link_kind {
        Some(rwf_lib::model::LinkKind::Junction) => "Junction",
        Some(rwf_lib::model::LinkKind::Symlink) => "Symlink",
        None if is_dir => "Directory",
        None => "File",
    };
    let type_str = if is_readonly {
        format!("{} (Read-only)", type_label)
    } else {
        type_label.to_string()
    };

    let mut rows: Vec<(&str, String)> = vec![
        ("Name", smart_truncate(file_name, w.saturating_sub(8), "…")),
        ("Path", smart_truncate(file_path, w.saturating_sub(8), "…")),
        ("Size", fmt_size(size)),
        ("Type", type_str),
    ];

    if let Some(target) = link_target {
        rows.push(("Target", smart_truncate(target, w.saturating_sub(8), "…")));
    }

    rows.push(("", String::new()));
    rows.push(("Created", fmt_time(created)));
    rows.push(("Modified", fmt_time(Some(modified))));
    rows.push(("Accessed", fmt_time(accessed)));

    let detected_line = if detecting {
        Some("Detecting...".to_string())
    } else {
        detected_type.map(|dt| format!("Detected type: {}", dt))
    };
    if detected_line.is_some() {
        rows.push(("", String::new()));
    }

    let col_w = 9u16; // label column width ("Modified" = 8 chars + space)
    for (row_i, (lbl, val)) in rows.iter().enumerate() {
        let y = area.y + row_i as u16;
        if y + 1 >= area.y + area.height {
            break;
        }
        if lbl.is_empty() {
            continue;
        }
        frame.render_widget(
            Paragraph::new(format!("{:<col_w$}", lbl, col_w = col_w as usize)).style(label),
            Rect::new(area.x + 2, y, col_w, 1),
        );
        frame.render_widget(
            Paragraph::new(val.as_str()).style(base),
            Rect::new(
                area.x + 2 + col_w,
                y,
                w.saturating_sub(col_w as usize) as u16,
                1,
            ),
        );
    }

    let mut next_row = rows.len() as u16;

    if let Some(text) = detected_line {
        let y = area.y + next_row;
        if y + 1 < area.y + area.height {
            frame.render_widget(
                Paragraph::new(text).style(base),
                Rect::new(area.x + 2, y, w as u16, 1),
            );
        }
        next_row += 1;
    }

    // Header-bytes audit view (Phase 7.3b, Task 10): up to 4 rows of the
    // leading bytes used for content-type detection, hex or raw text
    // depending on `header_hex_mode`. Nothing to show until detection has
    // completed successfully.
    if let Some(bytes) = header_bytes {
        if header_hex_mode {
            // Hex mode: unchanged from Task 10. `format_hex_row` is the same
            // helper the real hex viewer uses for its own ASCII column — a
            // hex view's ASCII column is legitimately byte-granular (one
            // glyph per raw byte, non-printable -> '.'), that's standard
            // hex-editor convention. Do not decode this as text.
            for (row_idx, chunk) in bytes.chunks(16).take(4).enumerate() {
                let y = area.y + next_row + row_idx as u16;
                if y + 1 >= area.y + area.height {
                    break;
                }
                let (offset, hex_str, ascii_str) =
                    rwf_lib::model::viewer::format_hex_row(row_idx * 16, chunk);
                let line = format!("{:06X}  {} {}", offset, hex_str, ascii_str);
                frame.render_widget(
                    Paragraph::new(line).style(base),
                    Rect::new(area.x + 2, y, w as u16, 1),
                );
            }
        } else {
            // Text mode: reuse the SAME decode pipeline as the real file
            // viewer's Text mode (rwf-bin/src/ui/viewer.rs) — detect the
            // encoding, decode, then sanitize control chars — instead of a
            // hand-rolled byte-to-char mapping. That old mapping treated
            // every byte as an independent ASCII-or-dot glyph, which can
            // never render CJK (a CJK char is 2-3 UTF-8/Shift-JIS/EUC-JP
            // bytes). Decoding the full byte window (not chunked to 16 bytes
            // first) lets multi-byte sequences that span a would-be 16-byte
            // boundary decode correctly.
            //
            // Known, accepted edge case: `header_bytes` is a raw truncated
            // window (first ~64 bytes of the file), not aligned to a
            // character boundary, so the trailing bytes may form a partial
            // multi-byte sequence at the cut-off. `TextEncoding::decode`
            // renders that as a replacement character (U+FFFD) rather than
            // panicking — expected behavior for any byte-window text
            // preview, not something to special-case here.
            let encoding = rwf_lib::model::viewer::TextEncoding::detect(bytes);
            let decoded = encoding.decode(bytes);
            let sanitized = sanitize_for_display(&decoded);
            for (row_idx, line) in wrap_decoded_text(&sanitized, w, 4).into_iter().enumerate() {
                let y = area.y + next_row + row_idx as u16;
                if y + 1 >= area.y + area.height {
                    break;
                }
                frame.render_widget(
                    Paragraph::new(line).style(base),
                    Rect::new(area.x + 2, y, w as u16, 1),
                );
            }
        }
    }

    // Hint line
    let hint_y = area.y + area.height.saturating_sub(1);
    let hint_text = if header_bytes.is_some() {
        "Enter/Esc: close  d: detect type  t: toggle hex/text"
    } else {
        "Enter/Esc: close  d: detect type"
    };
    frame.render_widget(
        Paragraph::new(hint_text).style(hint),
        Rect::new(area.x + 2, hint_y, w as u16, 1),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use rwf_lib::model::viewer::{format_hex_row, TextEncoding};

    /// This is the test that would have caught the original bug: the old
    /// byte-to-char mapping in text mode could never render CJK because it
    /// mapped each raw byte independently (a CJK char is multiple bytes).
    /// Decoding via the real viewer's `TextEncoding` pipeline must produce
    /// the actual CJK characters, not dots or replacement bytes.
    #[test]
    fn cjk_bytes_decode_to_actual_cjk_text() {
        let bytes = "こんにちは".as_bytes();
        let encoding = TextEncoding::detect(bytes);
        let decoded = encoding.decode(bytes);
        let sanitized = sanitize_for_display(&decoded);

        assert_eq!(sanitized, "こんにちは");

        let lines = wrap_decoded_text(&sanitized, 46, 4);
        let joined = lines.join("");
        assert_eq!(joined, "こんにちは");
        assert!(
            !joined.contains('.'),
            "CJK text must not be dot-mapped: {joined:?}"
        );
    }

    /// The whole point of routing through `TextEncoding::detect`/`.decode()`
    /// is to get non-UTF-8 Japanese encodings (Shift-JIS, EUC-JP, ...)
    /// "for free". Prove the File Info call site actually takes that branch
    /// — not an accidental hardcode to `TextEncoding::Utf8` — by feeding
    /// real Shift-JIS bytes through the exact same chain the render
    /// function uses (detect -> decode -> sanitize -> wrap).
    #[test]
    fn shift_jis_bytes_decode_through_the_full_chain() {
        let original = "こんにちは";
        let (encoded, _, had_errors) = encoding_rs::SHIFT_JIS.encode(original);
        assert!(
            !had_errors,
            "Shift-JIS encoding of the fixture must succeed"
        );
        let bytes = encoded.into_owned();

        // Sanity check: these bytes must NOT already be valid UTF-8, or this
        // test would pass even with a hardcoded Utf8 branch.
        assert!(
            std::str::from_utf8(&bytes).is_err(),
            "fixture must be genuinely non-UTF-8 to exercise the Shift-JIS branch"
        );

        let encoding = TextEncoding::detect(&bytes);
        assert_eq!(
            encoding,
            TextEncoding::ShiftJis,
            "detect() must recognize Shift-JIS rather than falling back to Utf8"
        );

        let decoded = encoding.decode(&bytes);
        let sanitized = sanitize_for_display(&decoded);
        assert_eq!(sanitized, original);

        let lines = wrap_decoded_text(&sanitized, 46, 4);
        assert_eq!(lines.join(""), original);
    }

    /// `header_bytes` is a raw truncated window (first ~64 bytes of the
    /// file), not aligned to a character boundary, so the trailing bytes
    /// may form a partial multi-byte sequence at the cut-off. This must not
    /// panic anywhere in the chain, and the incomplete sequence must render
    /// as a replacement character rather than corrupting the rest of the
    /// output.
    #[test]
    fn truncated_multibyte_sequence_decodes_to_replacement_char_without_panicking() {
        let original = "こんにちは"; // 5 chars, 3 bytes each in UTF-8
        let full_bytes = original.as_bytes();
        // Cut off the last byte of the final 3-byte character.
        let truncated = &full_bytes[..full_bytes.len() - 1];

        let encoding = TextEncoding::detect(truncated);
        let decoded = encoding.decode(truncated);
        let sanitized = sanitize_for_display(&decoded);

        // The 4 complete leading characters must survive intact.
        assert!(
            sanitized.starts_with("こんにち"),
            "leading complete characters must decode correctly: {sanitized:?}"
        );
        // The cut-off character must surface as U+FFFD, not silently vanish
        // or corrupt the preceding text.
        assert!(
            sanitized.contains('\u{FFFD}'),
            "expected a replacement character for the truncated sequence: {sanitized:?}"
        );

        // The wrap function must not choke on it — still produces valid,
        // fully-accounted-for line output.
        let lines = wrap_decoded_text(&sanitized, 46, 4);
        assert!(!lines.is_empty());
        assert_eq!(lines.join(""), sanitized);
    }

    /// Regression guard: hex mode must be byte-for-byte unchanged by the
    /// text-mode fix. Same helper (`format_hex_row`), same output.
    #[test]
    fn hex_mode_output_is_unchanged_by_the_text_mode_fix() {
        let bytes = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52,
        ];
        let (offset, hex_str, ascii_str) = format_hex_row(0, &bytes);
        let line = format!("{:06X}  {} {}", offset, hex_str, ascii_str);
        assert_eq!(
            line,
            "000000  89 50 4E 47 0D 0A 1A 0A  00 00 00 0D 49 48 44 52  .PNG........IHDR"
        );
    }

    #[test]
    fn wrap_is_width_aware_not_char_count_aware() {
        // 10 CJK characters (width 2 each = width 20) must wrap sooner than
        // 10 ASCII characters (width 1 each = width 10) at the same budget.
        let cjk = "日".repeat(10);
        let ascii = "a".repeat(10);

        let cjk_lines = wrap_decoded_text(&cjk, 10, 4);
        let ascii_lines = wrap_decoded_text(&ascii, 10, 4);

        // ASCII fits entirely on one line at width 10.
        assert_eq!(ascii_lines.len(), 1);
        assert_eq!(ascii_lines[0], ascii);

        // CJK at width 10 only fits 5 double-width chars per line, so 10
        // chars need 2 lines — proving the wrap counts display columns, not
        // character count.
        assert_eq!(cjk_lines.len(), 2);
        assert_eq!(cjk_lines[0].chars().count(), 5);
        assert_eq!(cjk_lines[1].chars().count(), 5);
    }

    #[test]
    fn wrap_truncates_with_ellipsis_past_max_lines() {
        let text = "a".repeat(50); // needs 5 lines at width 10, budget is 4
        let lines = wrap_decoded_text(&text, 10, 4);
        assert_eq!(lines.len(), 4);
        assert!(lines[3].ends_with('…'), "expected ellipsis: {:?}", lines[3]);
    }

    #[test]
    fn wrap_exact_fit_no_truncation() {
        let text = "a".repeat(40); // exactly 4 lines at width 10
        let lines = wrap_decoded_text(&text, 10, 4);
        assert_eq!(lines.len(), 4);
        assert!(!lines[3].contains('…'));
        assert_eq!(lines.join(""), text);
    }
}
