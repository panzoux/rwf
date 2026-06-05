//! File viewer state and operations
//!
//! Memory-mapped, windowed viewer: the file is never fully loaded into RAM.
//! A background job builds a line-offset index (Vec<u64>) progressively and
//! stores it behind an Arc<Mutex<LineIndex>> shared with the UI thread.
//! The renderer only decodes the visible viewport window on each frame.

use std::sync::{Arc, Mutex};
use regex::Regex;
use crate::model::Location;

// ──────────────────────────────────────────────────────────────────────────────
// FileBytes: mmap or in-memory bytes (for tests)
// ──────────────────────────────────────────────────────────────────────────────

pub enum FileBytes {
    /// Memory-mapped file — OS pages on demand, no RAM copy.
    Mapped(memmap2::Mmap),
    /// Small / test data held in-memory.
    InMemory(Vec<u8>),
}

impl FileBytes {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            FileBytes::Mapped(m) => m,
            FileBytes::InMemory(v) => v,
        }
    }
    pub fn len(&self) -> usize {
        match self {
            FileBytes::Mapped(m) => m.len(),
            FileBytes::InMemory(v) => v.len(),
        }
    }
    pub fn is_empty(&self) -> bool { self.len() == 0 }
}

impl std::fmt::Debug for FileBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileBytes({}B)", self.len())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LineIndex: byte offset of the start of each line
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct LineIndex {
    /// offsets[N] = byte offset of the first byte of line N.
    /// offsets[0] == 0 always (line 0 starts at the beginning).
    pub offsets: Vec<u64>,
    pub is_complete: bool,
}

impl LineIndex {
    pub fn new() -> Self {
        Self { offsets: vec![0], is_complete: false }
    }
    pub fn new_complete_empty() -> Self {
        Self { offsets: vec![0], is_complete: true }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ViewerBuffer: shared between the background indexer and the UI thread
// ──────────────────────────────────────────────────────────────────────────────

pub struct ViewerBuffer {
    pub bytes: Arc<FileBytes>,
    pub line_index: Arc<Mutex<LineIndex>>,
}

impl ViewerBuffer {
    pub fn new(bytes: FileBytes, line_index: LineIndex) -> Self {
        Self {
            bytes: Arc::new(bytes),
            line_index: Arc::new(Mutex::new(line_index)),
        }
    }

    pub fn total_bytes(&self) -> usize { self.bytes.len() }
}

impl Clone for ViewerBuffer {
    fn clone(&self) -> Self {
        Self {
            bytes: Arc::clone(&self.bytes),
            line_index: Arc::clone(&self.line_index),
        }
    }
}

impl std::fmt::Debug for ViewerBuffer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let indexed = self.line_index.lock().map(|l| l.offsets.len()).unwrap_or(0);
        write!(f, "ViewerBuffer({}B, {}lines)", self.bytes.len(), indexed)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ViewerState
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ViewerState {
    pub location: Location,
    pub mode: ViewerMode,
    pub buffer: Option<ViewerBuffer>,
    pub encoding: TextEncoding,
    pub line_offset: usize,
    pub column_offset: usize,
    pub search_query: Option<String>,
    pub search_match_index: Option<usize>,
    /// Each entry is (line_idx, byte_start, byte_end) in the decoded line string.
    pub search_matches: Vec<(usize, usize, usize)>,
    pub case_sensitive: bool,
    pub search_forward: bool,
    /// Decoded content width in display columns, updated by UpdatePaneWidth.
    pub content_width: usize,
    pub is_loading: bool,
    /// Set when the search was an address jump (hex mode). Holds the raw typed query
    /// so the renderer can highlight the matching digit suffix within the address label.
    pub address_query: Option<String>,
}

impl ViewerState {
    pub fn new(location: Location) -> Self {
        Self {
            location,
            mode: ViewerMode::Text,
            buffer: None,
            encoding: TextEncoding::Utf8,
            line_offset: 0,
            column_offset: 0,
            search_query: None,
            search_match_index: None,
            search_matches: Vec::new(),
            case_sensitive: false,
            search_forward: true,
            content_width: 70,
            is_loading: true,
            address_query: None,
        }
    }

    // ── Compat: used by tests and ViewerLoadComplete transition ───────────────

    /// Build an InMemory buffer from raw bytes. Builds the line index
    /// synchronously (complete). Used for tests and the legacy transition.
    pub fn set_contents(&mut self, contents: Vec<u8>) {
        let mut offsets: Vec<u64> = vec![0];
        for (i, &b) in contents.iter().enumerate() {
            if b == b'\n' {
                let next = (i + 1) as u64;
                if next < contents.len() as u64 {
                    offsets.push(next);
                }
            }
        }
        let line_index = LineIndex { offsets, is_complete: true };
        self.buffer = Some(ViewerBuffer::new(FileBytes::InMemory(contents), line_index));
        self.is_loading = false;
    }

    // ── Text access (used by tests, not by renderer) ──────────────────────────

    /// Returns the raw file bytes as a UTF-8 str. Only meaningful for InMemory
    /// buffers (returns None for memory-mapped files).
    pub fn text(&self) -> Option<&str> {
        match self.buffer.as_ref()?.bytes.as_ref() {
            FileBytes::InMemory(v) => std::str::from_utf8(v).ok(),
            FileBytes::Mapped(_) => None,
        }
    }

    // ── Line access ───────────────────────────────────────────────────────────

    /// Number of lines indexed so far. Grows while the background indexer runs.
    pub fn line_count(&self) -> usize {
        self.buffer.as_ref()
            .and_then(|b| b.line_index.lock().ok())
            .map(|idx| idx.offsets.len())
            .unwrap_or(0)
    }

    /// Whether the line index is fully built.
    pub fn is_index_complete(&self) -> bool {
        self.buffer.as_ref()
            .and_then(|b| b.line_index.lock().ok())
            .map(|idx| idx.is_complete)
            .unwrap_or(false)
    }

    /// Number of hex rows (16 bytes each) based on total file size.
    pub fn hex_line_count(&self) -> usize {
        self.buffer.as_ref()
            .map(|b| b.bytes.len().div_ceil(16))
            .unwrap_or(0)
    }

    /// Get the raw bytes for one line (trailing \r\n stripped).
    /// Capped at 64 KB to prevent decoding huge binary "lines".
    /// Returns None if line_idx is not yet indexed.
    pub fn get_line_bytes(&self, line_idx: usize) -> Option<Vec<u8>> {
        const MAX_LINE_BYTES: usize = 65536;
        let buffer = self.buffer.as_ref()?;
        let bytes = buffer.bytes.as_bytes();
        let index = buffer.line_index.lock().ok()?;
        if line_idx >= index.offsets.len() {
            return None;
        }
        let start = index.offsets[line_idx] as usize;
        let end = if line_idx + 1 < index.offsets.len() {
            index.offsets[line_idx + 1] as usize
        } else {
            bytes.len()
        };
        // Cap to avoid decoding megabytes of binary data as a single "line".
        let end = end.min(start + MAX_LINE_BYTES);
        drop(index);
        let mut raw = bytes[start..end].to_vec();
        while matches!(raw.last(), Some(&b'\n') | Some(&b'\r')) {
            raw.pop();
        }
        Some(raw)
    }

    /// Decode one line with the current encoding into a String.
    pub fn get_line_str(&self, line_idx: usize) -> Option<String> {
        let raw = self.get_line_bytes(line_idx)?;
        Some(self.encoding.decode(&raw))
    }

    // ── Hex access ────────────────────────────────────────────────────────────

    /// Raw bytes for one hex row (up to 16 bytes), for highlight rendering.
    pub fn get_hex_bytes_vec(&self, line_idx: usize) -> Option<(usize, Vec<u8>)> {
        let buffer = self.buffer.as_ref()?;
        let all = buffer.bytes.as_bytes();
        let offset = line_idx * 16;
        if offset >= all.len() { return None; }
        let end = (offset + 16).min(all.len());
        Some((offset, all[offset..end].to_vec()))
    }

    pub fn get_hex_line(&self, line_idx: usize) -> Option<(usize, String, String)> {
        let buffer = self.buffer.as_ref()?;
        let all = buffer.bytes.as_bytes();
        let total = all.len();
        let offset = line_idx * 16;
        if offset >= total { return None; }
        let end = (offset + 16).min(total);
        let bytes = &all[offset..end];

        let mut hex_str = String::new();
        for (i, byte) in bytes.iter().enumerate() {
            if i > 0 && i % 8 == 0 { hex_str.push(' '); }
            hex_str.push_str(&format!("{:02X} ", byte));
        }
        let padding = (16 - bytes.len()) * 3 + if bytes.len() <= 8 { 1 } else { 0 };
        hex_str.push_str(&" ".repeat(padding));

        let ascii_str: String = bytes.iter()
            .map(|&b| if (32..=126).contains(&b) { b as char } else { '.' })
            .collect();

        Some((offset, hex_str, ascii_str))
    }

    // ── Encoding ──────────────────────────────────────────────────────────────

    pub fn cycle_encoding(&mut self) {
        self.encoding = self.encoding.next();
        // Windowed renderer picks up the new encoding on the next frame.
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    pub fn move_to_line_start(&mut self) {
        self.column_offset = 0;
    }

    pub fn move_to_line_end(&mut self, viewport_width: usize) {
        if let Some(line) = self.get_line_str(self.line_offset) {
            if line.len() > viewport_width {
                self.column_offset = line.len().saturating_sub(viewport_width);
            } else {
                self.column_offset = 0;
            }
        }
    }

    pub fn jump_to_top(&mut self) {
        self.line_offset = 0;
        self.column_offset = 0;
    }

    pub fn jump_to_bottom(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset = line_count.saturating_sub(viewport_height);
        self.column_offset = 0;
    }

    pub fn scroll_down(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        if self.line_offset + viewport_height < line_count {
            self.line_offset += 1;
        }
    }

    pub fn scroll_up(&mut self) {
        if self.line_offset > 0 { self.line_offset -= 1; }
    }

    pub fn page_down(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset = (self.line_offset + viewport_height)
            .min(line_count.saturating_sub(viewport_height));
    }

    pub fn page_up(&mut self, viewport_height: usize) {
        self.line_offset = self.line_offset.saturating_sub(viewport_height);
    }

    pub fn scroll_left(&mut self, cols: usize) {
        self.column_offset = self.column_offset.saturating_sub(cols);
    }

    pub fn scroll_right(&mut self, cols: usize) {
        self.column_offset += cols;
    }

    pub fn fast_scroll_up(&mut self, lines: usize) {
        self.line_offset = self.line_offset.saturating_sub(lines);
    }

    pub fn fast_scroll_down(&mut self, lines: usize, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset = (self.line_offset + lines)
            .min(line_count.saturating_sub(viewport_height));
    }

    // ── Search ────────────────────────────────────────────────────────────────

    /// Run a search. `migemo_pattern` is a pre-built regex string from the migemo
    /// library; when present it overrides the plain-text query for matching but
    /// the query is still stored for display purposes.
    pub fn start_search(&mut self, query: String, migemo_pattern: Option<&str>) {
        self.search_query = Some(query.clone());
        self.search_matches.clear();
        self.search_match_index = None;
        self.address_query = None;

        if query.is_empty() { return; }

        if self.mode == ViewerMode::Hex {
            self.start_hex_search(&query);
            return;
        }

        // Build regex: migemo pattern > plain escaped query with case flag.
        let pattern = if let Some(mp) = migemo_pattern {
            mp.to_string()
        } else if self.case_sensitive {
            regex::escape(&query)
        } else {
            format!("(?i){}", regex::escape(&query))
        };
        let re = match Regex::new(&pattern) {
            Ok(r) => r,
            Err(_) => return,
        };

        if let Some(ref buffer) = self.buffer {
            let bytes = buffer.bytes.as_bytes();
            let index = buffer.line_index.lock().unwrap();
            let n = index.offsets.len();

            for line_idx in 0..n {
                let start = index.offsets[line_idx] as usize;
                let end = if line_idx + 1 < n {
                    index.offsets[line_idx + 1] as usize
                } else {
                    bytes.len()
                };
                let raw = trim_newline(&bytes[start..end.min(start + 65536)]);
                let line = self.encoding.decode(raw);
                for m in re.find_iter(&line) {
                    self.search_matches.push((line_idx, m.start(), m.end()));
                }
            }
        }

        if !self.search_matches.is_empty() {
            let start_idx = if self.search_forward {
                self.search_matches.iter().position(|&(l, _, _)| l >= self.line_offset)
                    .unwrap_or(0)
            } else {
                self.search_matches.iter().rposition(|&(l, _, _)| l <= self.line_offset)
                    .unwrap_or(self.search_matches.len() - 1)
            };
            self.search_match_index = Some(start_idx);
            self.jump_to_match(start_idx);
        }
    }

    /// Hex-mode search: parses query as address, hex byte pattern, or text.
    fn start_hex_search(&mut self, query: &str) {
        let buffer = match self.buffer.as_ref() {
            Some(b) => b,
            None => return,
        };
        let all_bytes = buffer.bytes.clone();
        let file_bytes = all_bytes.as_bytes();

        let byte_hits: Option<Vec<(usize, usize)>> = match parse_hex_query(query) {
            HexSearchPattern::Address(addr) => {
                if addr < file_bytes.len() {
                    self.line_offset = addr / 16;
                    // Track so the renderer highlights the matching digit suffix
                    // in the address label column.
                    self.address_query = Some(query.to_string());
                }
                // Derive a byte pattern from the even-length hex digits so the
                // matching bytes are highlighted in the hex/ASCII data too.
                let digits = query.trim()
                    .strip_prefix("0x").or_else(|| query.trim().strip_prefix("0X"))
                    .unwrap_or(query.trim());
                if digits.len() >= 2 && digits.len() % 2 == 0
                    && digits.chars().all(|c| c.is_ascii_hexdigit())
                {
                    let needle: Option<Vec<u8>> = (0..digits.len())
                        .step_by(2)
                        .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok())
                        .collect();
                    needle.map(|n| find_byte_pattern(file_bytes, &n))
                } else {
                    None
                }
            }
            HexSearchPattern::Bytes(needle) => {
                Some(find_byte_pattern(file_bytes, &needle))
            }
            HexSearchPattern::Text(needle) => {
                let hits = if self.case_sensitive {
                    find_byte_pattern(file_bytes, &needle)
                } else {
                    let lower: Vec<u8> = needle.iter().map(|b| b.to_ascii_lowercase()).collect();
                    find_byte_pattern_ci(file_bytes, &lower)
                };
                Some(hits)
            }
        };

        if let Some(hits) = byte_hits {
            for (s, e) in hits {
                self.search_matches.push((s / 16, s, e));
            }
            if !self.search_matches.is_empty() {
                let cur_offset = self.line_offset * 16;
                let start_idx = if self.search_forward {
                    self.search_matches.iter().position(|&(_, s, _)| s >= cur_offset).unwrap_or(0)
                } else {
                    self.search_matches.iter().rposition(|&(_, s, _)| s < cur_offset + 16)
                        .unwrap_or(self.search_matches.len() - 1)
                };
                self.search_match_index = Some(start_idx);
                // Address searches already set line_offset to the typed address;
                // don't override it by jumping to the nearest byte match.
                if self.address_query.is_none() {
                    self.jump_to_match(start_idx);
                }
            }
        }
    }

    // ── Hex byte access ───────────────────────────────────────────────────────

    pub fn find_next(&mut self) {
        if self.search_matches.is_empty() { return; }
        let next = match self.search_match_index {
            Some(idx) => (idx + 1) % self.search_matches.len(),
            None => 0,
        };
        self.search_match_index = Some(next);
        self.jump_to_match(next);
    }

    pub fn find_prev(&mut self) {
        if self.search_matches.is_empty() { return; }
        let prev = match self.search_match_index {
            Some(idx) if idx > 0 => idx - 1,
            _ => self.search_matches.len() - 1,
        };
        self.search_match_index = Some(prev);
        self.jump_to_match(prev);
    }

    /// `n` key: forward in search direction, backward if search_forward=false.
    pub fn find_next_in_dir(&mut self) {
        if self.search_forward { self.find_next() } else { self.find_prev() }
    }

    /// `N` key: backward in search direction, forward if search_forward=false.
    pub fn find_prev_in_dir(&mut self) {
        if self.search_forward { self.find_prev() } else { self.find_next() }
    }

    fn jump_to_match(&mut self, match_idx: usize) {
        if let Some(&(line, byte_start, byte_end)) = self.search_matches.get(match_idx) {
            self.line_offset = line;
            if self.mode == ViewerMode::Hex { return; } // hex is fixed-width, no horiz scroll
            let cw = self.content_width.max(1);
            let col = self.column_offset;
            if byte_end <= cw {
                self.column_offset = 0;
            } else if byte_start >= col && byte_end <= col + cw {
                // Fully visible — no scroll.
            } else if byte_start < col {
                self.column_offset = byte_start;
            } else {
                self.column_offset = byte_end.saturating_sub(cw);
            }
        }
    }

    pub fn clear_search(&mut self) {
        self.search_query = None;
        self.search_matches.clear();
        self.search_match_index = None;
        self.address_query = None;
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Hex search helpers
// ──────────────────────────────────────────────────────────────────────────────

enum HexSearchPattern {
    Address(usize),
    Bytes(Vec<u8>),
    Text(Vec<u8>),
}

/// Auto-detect hex query type:
/// - "0x…" or "0X…" → Address
/// - Pure hex digits (no spaces) → Address (e.g. "0230", "1A4F")
/// - Hex digits with spaces → Bytes pattern (e.g. "4D 5A 90")
/// - Anything else → Text (raw UTF-8 bytes, case-insensitive by default)
fn parse_hex_query(query: &str) -> HexSearchPattern {
    let trimmed = query.trim();

    // Address: explicit 0x/0X prefix
    if let Some(rest) = trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(addr) = usize::from_str_radix(rest, 16) {
                return HexSearchPattern::Address(addr);
            }
        }
    }

    // Pure hex digits (no spaces): treat as address
    let all_hex = !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    if all_hex {
        if let Ok(addr) = usize::from_str_radix(trimmed, 16) {
            return HexSearchPattern::Address(addr);
        }
    }

    // Hex digits with spaces: byte pattern (e.g. "4D 5A 90")
    let has_space = trimmed.contains(' ');
    let all_hex_space = trimmed.chars().all(|c| c.is_ascii_hexdigit() || c == ' ');
    if all_hex_space && has_space && !trimmed.is_empty() {
        let no_space: String = trimmed.chars().filter(|&c| c != ' ').collect();
        if no_space.len() >= 2 && no_space.len() % 2 == 0 {
            let bytes: Option<Vec<u8>> = (0..no_space.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&no_space[i..i + 2], 16).ok())
                .collect();
            if let Some(b) = bytes {
                return HexSearchPattern::Bytes(b);
            }
        }
    }

    // Text: search for raw UTF-8 bytes
    HexSearchPattern::Text(trimmed.as_bytes().to_vec())
}

/// Simple linear byte-pattern search. Returns (start, end) file offsets.
fn find_byte_pattern(haystack: &[u8], needle: &[u8]) -> Vec<(usize, usize)> {
    if needle.is_empty() { return vec![]; }
    let mut result = Vec::new();
    let n = needle.len();
    if haystack.len() < n { return result; }
    for i in 0..=haystack.len() - n {
        if &haystack[i..i + n] == needle {
            result.push((i, i + n));
        }
    }
    result
}

/// Case-insensitive byte-pattern search. `lower_needle` must already be lowercased.
fn find_byte_pattern_ci(haystack: &[u8], lower_needle: &[u8]) -> Vec<(usize, usize)> {
    if lower_needle.is_empty() { return vec![]; }
    let n = lower_needle.len();
    if haystack.len() < n { return vec![]; }
    let mut result = Vec::new();
    for i in 0..=haystack.len() - n {
        if lower_needle.iter().zip(&haystack[i..i + n])
            .all(|(&a, &b)| a == b.to_ascii_lowercase())
        {
            result.push((i, i + n));
        }
    }
    result
}

fn trim_newline(raw: &[u8]) -> &[u8] {
    let mut end = raw.len();
    while end > 0 && (raw[end - 1] == b'\n' || raw[end - 1] == b'\r') {
        end -= 1;
    }
    &raw[..end]
}

// ──────────────────────────────────────────────────────────────────────────────
// ViewerMode
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewerMode {
    Text,
    Hex,
}

// ──────────────────────────────────────────────────────────────────────────────
// TextEncoding
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextEncoding {
    Utf8, Utf16Le, Utf16Be, ShiftJis, EucJp, Iso8859_1, Windows1252,
}

impl TextEncoding {
    pub fn decode(&self, bytes: &[u8]) -> String {
        match self {
            TextEncoding::Utf8    => String::from_utf8_lossy(bytes).into_owned(),
            TextEncoding::Utf16Le => decode_utf16_le(bytes),
            TextEncoding::Utf16Be => decode_utf16_be(bytes),
            TextEncoding::ShiftJis => {
                let (decoded, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(bytes);
                decoded.into_owned()
            }
            TextEncoding::EucJp => {
                let (decoded, _) = encoding_rs::EUC_JP.decode_without_bom_handling(bytes);
                decoded.into_owned()
            }
            TextEncoding::Iso8859_1 => {
                let (decoded, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
                decoded.into_owned()
            }
            TextEncoding::Windows1252 => {
                let (decoded, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
                decoded.into_owned()
            }
        }
    }

    /// Heuristic encoding detection from a byte sample (first 8–16 KB is enough).
    /// Priority: BOM → UTF-8 validity → Japanese statistical analysis → Latin-1.
    pub fn detect(bytes: &[u8]) -> TextEncoding {
        // BOM detection
        if bytes.starts_with(b"\xFF\xFE") { return TextEncoding::Utf16Le; }
        if bytes.starts_with(b"\xFE\xFF") { return TextEncoding::Utf16Be; }
        let payload = if bytes.starts_with(b"\xEF\xBB\xBF") { &bytes[3..] } else { bytes };
        if bytes.starts_with(b"\xEF\xBB\xBF") { return TextEncoding::Utf8; }

        // Strict UTF-8 check
        if std::str::from_utf8(payload).is_ok() { return TextEncoding::Utf8; }

        // Statistical analysis for Japanese encodings on a 4 KB sample.
        let sample = &payload[..payload.len().min(4096)];
        let mut sjis: i32 = 0;
        let mut eucjp: i32 = 0;
        let mut i = 0;
        while i < sample.len() {
            let b = sample[i];
            // Shift-JIS lead bytes: 0x81–0x9F or 0xE0–0xFC
            if (b >= 0x81 && b <= 0x9F) || (b >= 0xE0 && b <= 0xFC) {
                if i + 1 < sample.len() {
                    let b2 = sample[i + 1];
                    if (b2 >= 0x40 && b2 <= 0x7E) || (b2 >= 0x80 && b2 <= 0xFC) {
                        sjis += 2; i += 2; continue;
                    }
                }
                sjis -= 1;
            }
            // EUC-JP lead bytes: 0xA1–0xFE (and SS2 0x8E, SS3 0x8F)
            if b >= 0xA1 && b <= 0xFE {
                if i + 1 < sample.len() && sample[i + 1] >= 0xA1 && sample[i + 1] <= 0xFE {
                    eucjp += 2; i += 2; continue;
                }
                eucjp -= 1;
            }
            if b == 0x8E && i + 1 < sample.len() && sample[i + 1] >= 0xA1 {
                // SS2 half-width kana
                eucjp += 1; i += 2; continue;
            }
            i += 1;
        }
        if sjis > 0 || eucjp > 0 {
            if sjis >= eucjp { TextEncoding::ShiftJis } else { TextEncoding::EucJp }
        } else {
            TextEncoding::Windows1252
        }
    }

    pub fn next(&self) -> Self {
        match self {
            TextEncoding::Utf8        => TextEncoding::Utf16Le,
            TextEncoding::Utf16Le     => TextEncoding::Utf16Be,
            TextEncoding::Utf16Be     => TextEncoding::ShiftJis,
            TextEncoding::ShiftJis    => TextEncoding::EucJp,
            TextEncoding::EucJp       => TextEncoding::Iso8859_1,
            TextEncoding::Iso8859_1   => TextEncoding::Windows1252,
            TextEncoding::Windows1252 => TextEncoding::Utf8,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            TextEncoding::Utf8        => "UTF-8",
            TextEncoding::Utf16Le     => "UTF-16 LE",
            TextEncoding::Utf16Be     => "UTF-16 BE",
            TextEncoding::ShiftJis    => "Shift-JIS",
            TextEncoding::EucJp       => "EUC-JP",
            TextEncoding::Iso8859_1   => "ISO-8859-1",
            TextEncoding::Windows1252 => "Windows-1252",
        }
    }

    /// Decode bytes for one hex row into (display_char, byte_start, byte_end) tuples.
    /// Byte offsets are relative to the `bytes` slice. Used for encoding-aware ASCII section.
    pub fn decode_row_chars(&self, bytes: &[u8]) -> Vec<(char, usize, usize)> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < bytes.len() {
            let (raw_ch, consumed) = match self {
                TextEncoding::Utf8 => {
                    let b = bytes[i];
                    let seq_len = if b < 0x80 { 1 }
                        else if b < 0xC0 { 1 }
                        else if b < 0xE0 { 2 }
                        else if b < 0xF0 { 3 }
                        else { 4 };
                    let end = (i + seq_len).min(bytes.len());
                    let ch = std::str::from_utf8(&bytes[i..end])
                        .ok()
                        .and_then(|s| s.chars().next())
                        .unwrap_or('\u{FFFD}');
                    (ch, end - i)
                }
                TextEncoding::Utf16Le => {
                    if i + 1 < bytes.len() {
                        let code = u16::from_le_bytes([bytes[i], bytes[i + 1]]);
                        (char::from_u32(code as u32).unwrap_or('\u{FFFD}'), 2)
                    } else {
                        ('\u{FFFD}', 1)
                    }
                }
                TextEncoding::Utf16Be => {
                    if i + 1 < bytes.len() {
                        let code = u16::from_be_bytes([bytes[i], bytes[i + 1]]);
                        (char::from_u32(code as u32).unwrap_or('\u{FFFD}'), 2)
                    } else {
                        ('\u{FFFD}', 1)
                    }
                }
                TextEncoding::ShiftJis => {
                    let b = bytes[i];
                    let is_lead = (b >= 0x81 && b <= 0x9F) || (b >= 0xE0 && b <= 0xFC);
                    if is_lead && i + 1 < bytes.len() {
                        let (s, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if is_lead {
                        ('.', 1)
                    } else if (0xA1..=0xDF).contains(&b) {
                        let (s, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes[i..i + 1]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 1)
                    } else {
                        (if (32..=126).contains(&b) { b as char } else { '\u{FFFD}' }, 1)
                    }
                }
                TextEncoding::EucJp => {
                    let b = bytes[i];
                    if (0xA1..=0xFE).contains(&b) && i + 1 < bytes.len() {
                        let (s, _) = encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if b == 0x8E && i + 1 < bytes.len() {
                        let (s, _) = encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if b == 0x8F && i + 2 < bytes.len() {
                        let (s, _) = encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 3]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 3)
                    } else {
                        (if (32..=126).contains(&b) { b as char } else { '\u{FFFD}' }, 1)
                    }
                }
                TextEncoding::Iso8859_1 | TextEncoding::Windows1252 => {
                    let (s, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(&bytes[i..i + 1]);
                    (s.chars().next().unwrap_or('\u{FFFD}'), 1)
                }
            };
            let display = if raw_ch.is_control() || raw_ch == '\u{FFFD}' { '.' } else { raw_ch };
            result.push((display, i, i + consumed));
            i += consumed;
        }
        result
    }
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes.chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes.chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> Location { Location::Local(std::path::PathBuf::from("/test/file.txt")) }

    #[test]
    fn test_viewer_state_creation() {
        let v = ViewerState::new(loc());
        assert_eq!(v.mode, ViewerMode::Text);
        assert_eq!(v.encoding, TextEncoding::Utf8);
        assert_eq!(v.line_offset, 0);
        assert!(v.is_loading);
    }

    #[test]
    fn test_set_contents_and_decode() {
        let mut v = ViewerState::new(loc());
        v.set_contents(b"Hello\nWorld\n".to_vec());
        assert_eq!(v.text(), Some("Hello\nWorld\n"));
        assert_eq!(v.line_count(), 2);
        assert!(!v.is_loading);
    }

    #[test]
    fn test_encoding_cycle() {
        let mut v = ViewerState::new(loc());
        assert_eq!(v.encoding, TextEncoding::Utf8);
        v.cycle_encoding();
        assert_eq!(v.encoding, TextEncoding::Utf16Le);
        v.cycle_encoding();
        assert_eq!(v.encoding, TextEncoding::Utf16Be);
    }

    #[test]
    fn test_navigation() {
        let mut v = ViewerState::new(loc());
        v.set_contents(b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec());

        v.line_offset = 3;
        v.jump_to_top();
        assert_eq!(v.line_offset, 0);

        v.jump_to_bottom(2);
        assert!(v.line_offset > 0);

        v.line_offset = 2;
        v.scroll_down(10);
        assert_eq!(v.line_offset, 2); // can't scroll past end

        v.scroll_up();
        assert_eq!(v.line_offset, 1);
    }

    #[test]
    fn test_search() {
        let mut v = ViewerState::new(loc());
        v.set_contents(b"Hello World\nHello Rust\nGoodbye World\n".to_vec());

        v.start_search("Hello".to_string(), None);
        assert_eq!(v.search_matches.len(), 2);
        assert_eq!(v.search_match_index, Some(0));

        v.find_next();
        assert_eq!(v.search_match_index, Some(1));

        v.find_prev();
        assert_eq!(v.search_match_index, Some(0));
    }

    #[test]
    fn test_utf8_decoding() {
        let decoded = TextEncoding::Utf8.decode("Hello 世界".as_bytes());
        assert_eq!(decoded, "Hello 世界");
    }

    #[test]
    fn test_iso8859_1_decoding() {
        let decoded = TextEncoding::Iso8859_1.decode(b"Hello \xE9");
        assert_eq!(decoded, "Hello é");
    }

    #[test]
    fn test_hex_viewer() {
        let mut v = ViewerState::new(Location::Local(std::path::PathBuf::from("/test/file.bin")));
        v.mode = ViewerMode::Hex;
        v.set_contents(vec![
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F,
            0x72, 0x6C, 0x64, 0x21, 0x00, 0xFF, 0xAA, 0x55,
            0x01, 0x02, 0x03,
        ]);
        assert_eq!(v.hex_line_count(), 2);
        let (offset, hex, ascii) = v.get_hex_line(0).unwrap();
        assert_eq!(offset, 0);
        assert!(hex.contains("48 65 6C 6C"));
        assert!(ascii.contains("Hello"));

        let (offset, hex, _) = v.get_hex_line(1).unwrap();
        assert_eq!(offset, 16);
        assert!(hex.contains("01 02 03"));
    }

    #[test]
    fn test_hex_navigation() {
        let mut v = ViewerState::new(Location::Local(std::path::PathBuf::from("/test/file.bin")));
        v.mode = ViewerMode::Hex;
        v.set_contents(vec![0u8; 100]);
        assert_eq!(v.hex_line_count(), 7);
        v.scroll_down(5);
        assert_eq!(v.line_offset, 1);
        v.jump_to_bottom(5);
        assert_eq!(v.line_offset, 2);
    }
}
