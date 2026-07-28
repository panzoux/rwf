//! File viewer state and operations
//!
//! Windowed viewer: the file is never fully loaded into RAM for large files.
//! A background job builds a line-offset index (Vec<u64>) progressively and
//! stores it behind an Arc<Mutex<LineIndex>> shared with the UI thread.
//! The renderer only decodes the visible viewport window on each frame.

use crate::model::Location;
use regex::Regex;
use std::sync::{Arc, Mutex};

// ──────────────────────────────────────────────────────────────────────────────
// SeekableFile: File + Seek + Read (no mmap). Used for files above threshold.
// ──────────────────────────────────────────────────────────────────────────────

/// Large file handle using seek+read on demand (thread-safe)
pub struct SeekableFile {
    file: Arc<Mutex<std::fs::File>>,
    pub size: u64,
}

impl SeekableFile {
    /// Create a new seekable file handle wrapping the given file object
    pub fn new(file: std::fs::File, size: u64) -> Self {
        Self {
            file: Arc::new(Mutex::new(file)),
            size,
        }
    }

    /// Read `len` bytes starting at `offset`. Returns fewer bytes if near EOF.
    pub fn read_bytes(&self, offset: u64, len: usize) -> std::io::Result<Vec<u8>> {
        use std::io::{Read, Seek, SeekFrom};
        let available = (self.size.saturating_sub(offset)) as usize;
        let to_read = len.min(available);
        if to_read == 0 {
            return Ok(vec![]);
        }
        let mut f = self
            .file
            .lock()
            .expect("SeekableFile mutex should not be poisoned");
        f.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0u8; to_read];
        let mut total = 0;
        while total < to_read {
            let n = f.read(&mut buf[total..])?;
            if n == 0 {
                break;
            }
            total += n;
        }
        buf.truncate(total);
        Ok(buf)
    }
}

impl std::fmt::Debug for SeekableFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SeekableFile({}B)", self.size)
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// FileBytes: in-memory bytes or seekable file handle
// ──────────────────────────────────────────────────────────────────────────────

/// File contents: either in-memory bytes or a seekable file handle
pub enum FileBytes {
    /// Small files (≤ threshold): entire contents held in RAM. Snapshot is stable.
    InMemory(Vec<u8>),
    /// Large files (> threshold): seek+read on demand; no mmap page-fault risk.
    Seekable(SeekableFile),
}

impl FileBytes {
    /// Get the raw bytes (only valid for InMemory; panics on Seekable)
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            FileBytes::InMemory(v) => v,
            FileBytes::Seekable(_) => {
                unreachable!("Use SeekableFile::read_bytes() for Seekable files")
            }
        }
    }
    /// Get the total size in bytes
    pub fn len(&self) -> usize {
        match self {
            FileBytes::InMemory(v) => v.len(),
            FileBytes::Seekable(s) => s.size as usize,
        }
    }
    /// Check if the file is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl std::fmt::Debug for FileBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "FileBytes({}B)", self.len())
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// LineIndex: byte offset of the start of each line
// ──────────────────────────────────────────────────────────────────────────────

/// Index of line boundaries in a file (built progressively by the indexer)
#[derive(Debug)]
pub struct LineIndex {
    /// offsets[N] = byte offset of the first byte of line N.
    /// offsets[0] == 0 always (line 0 starts at the beginning).
    pub offsets: Vec<u64>,
    pub is_complete: bool,
}

impl Default for LineIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl LineIndex {
    /// Create a new empty line index for building
    pub fn new() -> Self {
        Self {
            offsets: vec![0],
            is_complete: false,
        }
    }
    /// Create a complete line index for an empty file
    pub fn new_complete_empty() -> Self {
        Self {
            offsets: vec![0],
            is_complete: true,
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// ViewerBuffer: shared between the background indexer and the UI thread
// ──────────────────────────────────────────────────────────────────────────────

/// Shared file content and line index (thread-safe, shared with background job)
pub struct ViewerBuffer {
    pub bytes: Arc<FileBytes>,
    pub line_index: Arc<Mutex<LineIndex>>,
}

impl ViewerBuffer {
    /// Create a new shared viewer buffer
    pub fn new(bytes: FileBytes, line_index: LineIndex) -> Self {
        Self {
            bytes: Arc::new(bytes),
            line_index: Arc::new(Mutex::new(line_index)),
        }
    }

    /// Get the total file size in bytes
    pub fn total_bytes(&self) -> usize {
        self.bytes.len()
    }
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

/// Complete state for the file viewer (text/hex mode with search and navigation)
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
    /// True while a background search job is running.
    pub is_searching: bool,
}

impl ViewerState {
    /// Create a new viewer state for the specified location (starts in loading state)
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
            is_searching: false,
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
        let line_index = LineIndex {
            offsets,
            is_complete: true,
        };
        self.buffer = Some(ViewerBuffer::new(FileBytes::InMemory(contents), line_index));
        self.is_loading = false;
    }

    // ── Text access (used by tests, not by renderer) ──────────────────────────

    /// Returns the raw file bytes as a UTF-8 str. Only meaningful for InMemory
    /// buffers (returns None for Seekable files).
    pub fn text(&self) -> Option<&str> {
        match self.buffer.as_ref()?.bytes.as_ref() {
            FileBytes::InMemory(v) => std::str::from_utf8(v).ok(),
            FileBytes::Seekable(_) => None,
        }
    }

    // ── Line access ───────────────────────────────────────────────────────────

    /// Number of lines indexed so far. Grows while the background indexer runs.
    pub fn line_count(&self) -> usize {
        self.buffer
            .as_ref()
            .and_then(|b| b.line_index.lock().ok())
            .map(|idx| idx.offsets.len())
            .unwrap_or(0)
    }

    /// Whether the line index is fully built.
    pub fn is_index_complete(&self) -> bool {
        self.buffer
            .as_ref()
            .and_then(|b| b.line_index.lock().ok())
            .map(|idx| idx.is_complete)
            .unwrap_or(false)
    }

    /// Number of hex rows (16 bytes each) based on total file size.
    pub fn hex_line_count(&self) -> usize {
        self.buffer
            .as_ref()
            .map(|b| b.bytes.len().div_ceil(16))
            .unwrap_or(0)
    }

    /// Get the raw bytes for one line (trailing \r\n stripped).
    /// Capped at 64 KB to prevent decoding huge binary "lines".
    /// Returns None if line_idx is not yet indexed.
    pub fn get_line_bytes(&self, line_idx: usize) -> Option<Vec<u8>> {
        const MAX_LINE_BYTES: usize = 65536;
        let buffer = self.buffer.as_ref()?;
        // Release the index lock before any file I/O so Seekable reads don't deadlock.
        let (start, end) = {
            let index = buffer.line_index.lock().ok()?;
            if line_idx >= index.offsets.len() {
                return None;
            }
            let start = index.offsets[line_idx] as usize;
            let end = if line_idx + 1 < index.offsets.len() {
                index.offsets[line_idx + 1] as usize
            } else {
                buffer.bytes.len()
            };
            (start, end.min(start + MAX_LINE_BYTES))
        };
        let mut raw = match buffer.bytes.as_ref() {
            FileBytes::InMemory(v) => v[start..end].to_vec(),
            FileBytes::Seekable(s) => s.read_bytes(start as u64, end - start).ok()?,
        };
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
        let total = buffer.bytes.len();
        let offset = line_idx * 16;
        if offset >= total {
            return None;
        }
        let end = (offset + 16).min(total);
        let bytes = match buffer.bytes.as_ref() {
            FileBytes::InMemory(v) => v[offset..end].to_vec(),
            FileBytes::Seekable(s) => s.read_bytes(offset as u64, end - offset).ok()?,
        };
        Some((offset, bytes))
    }

    // ── Encoding ──────────────────────────────────────────────────────────────

    /// Switch to the next text encoding in the cycle
    pub fn cycle_encoding(&mut self) {
        self.encoding = self.encoding.next();
        // Windowed renderer picks up the new encoding on the next frame.
    }

    // ── Navigation ────────────────────────────────────────────────────────────

    /// Move to the start of the current line
    pub fn move_to_line_start(&mut self) {
        self.column_offset = 0;
    }

    /// Move to the end of the current line (respecting viewport width)
    pub fn move_to_line_end(&mut self, viewport_width: usize) {
        if let Some(line) = self.get_line_str(self.line_offset) {
            if line.len() > viewport_width {
                self.column_offset = line.len().saturating_sub(viewport_width);
            } else {
                self.column_offset = 0;
            }
        }
    }

    /// Jump to the first line
    pub fn jump_to_top(&mut self) {
        self.line_offset = 0;
        self.column_offset = 0;
    }

    /// Jump to the last line (respecting viewport height)
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
        if self.line_offset > 0 {
            self.line_offset -= 1;
        }
    }

    pub fn page_down(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset =
            (self.line_offset + viewport_height).min(line_count.saturating_sub(viewport_height));
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
        self.line_offset =
            (self.line_offset + lines).min(line_count.saturating_sub(viewport_height));
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

        if query.is_empty() {
            return;
        }

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

        // Collect line count without holding the index lock during reads
        // (Seekable reads also take a lock; holding the index lock would deadlock).
        let n = self.line_count();
        let mut new_matches: Vec<(usize, usize, usize)> = Vec::new();
        for line_idx in 0..n {
            let raw = match self.get_line_bytes(line_idx) {
                Some(b) => b,
                None => continue,
            };
            let line = self.encoding.decode(&raw);
            for m in re.find_iter(&line) {
                new_matches.push((line_idx, m.start(), m.end()));
            }
        }
        self.search_matches = new_matches;

        if !self.search_matches.is_empty() {
            let start_idx = if self.search_forward {
                self.search_matches
                    .iter()
                    .position(|&(l, _, _)| l >= self.line_offset)
                    .unwrap_or(0)
            } else {
                self.search_matches
                    .iter()
                    .rposition(|&(l, _, _)| l <= self.line_offset)
                    .unwrap_or(self.search_matches.len() - 1)
            };
            self.search_match_index = Some(start_idx);
            self.jump_to_match(start_idx);
        }
    }

    /// Hex-mode search: parses query as address, hex byte pattern, or text.
    /// Works for both InMemory (slice-based) and Seekable (chunked read) files.
    fn start_hex_search(&mut self, query: &str) {
        let buffer = match self.buffer.as_ref() {
            Some(b) => b,
            None => return,
        };

        let file_size = buffer.bytes.len();
        let bytes_ref = buffer.bytes.clone(); // Arc clone — cheap

        let byte_hits: Option<Vec<(usize, usize)>> = match bytes_ref.as_ref() {
            // ── InMemory: direct slice search ───────────────────────────────
            FileBytes::InMemory(v) => {
                let file_bytes = v.as_slice();
                match parse_hex_query(query) {
                    HexSearchPattern::Address(addr) => {
                        if addr < file_bytes.len() {
                            self.line_offset = addr / 16;
                            self.address_query = Some(query.to_string());
                        }
                        let digits = query
                            .trim()
                            .strip_prefix("0x")
                            .or_else(|| query.trim().strip_prefix("0X"))
                            .unwrap_or(query.trim());
                        if digits.len() >= 2
                            && digits.len().is_multiple_of(2)
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
                    HexSearchPattern::Bytes(needle) => Some(find_byte_pattern(file_bytes, &needle)),
                    HexSearchPattern::Text(needle) => {
                        let hits = if self.case_sensitive {
                            find_byte_pattern(file_bytes, &needle)
                        } else {
                            let lower: Vec<u8> =
                                needle.iter().map(|b| b.to_ascii_lowercase()).collect();
                            find_byte_pattern_ci(file_bytes, &lower)
                        };
                        Some(hits)
                    }
                }
            }
            // ── Seekable: chunked read search ────────────────────────────────
            FileBytes::Seekable(s) => match parse_hex_query(query) {
                HexSearchPattern::Address(addr) => {
                    if addr < file_size {
                        self.line_offset = addr / 16;
                        self.address_query = Some(query.to_string());
                    }
                    let digits = query
                        .trim()
                        .strip_prefix("0x")
                        .or_else(|| query.trim().strip_prefix("0X"))
                        .unwrap_or(query.trim());
                    if digits.len() >= 2
                        && digits.len().is_multiple_of(2)
                        && digits.chars().all(|c| c.is_ascii_hexdigit())
                    {
                        let needle: Option<Vec<u8>> = (0..digits.len())
                            .step_by(2)
                            .map(|i| u8::from_str_radix(&digits[i..i + 2], 16).ok())
                            .collect();
                        needle.map(|n| search_seekable_bytes(s, &n, false))
                    } else {
                        None
                    }
                }
                HexSearchPattern::Bytes(needle) => Some(search_seekable_bytes(s, &needle, false)),
                HexSearchPattern::Text(needle) => {
                    Some(search_seekable_bytes(s, &needle, !self.case_sensitive))
                }
            },
        };

        if let Some(hits) = byte_hits {
            for (s, e) in hits {
                self.search_matches.push((s / 16, s, e));
            }
            if !self.search_matches.is_empty() {
                let cur_offset = self.line_offset * 16;
                let start_idx = if self.search_forward {
                    self.search_matches
                        .iter()
                        .position(|&(_, s, _)| s >= cur_offset)
                        .unwrap_or(0)
                } else {
                    self.search_matches
                        .iter()
                        .rposition(|&(_, s, _)| s < cur_offset + 16)
                        .unwrap_or(self.search_matches.len() - 1)
                };
                self.search_match_index = Some(start_idx);
                // Address searches already set line_offset; don't override with byte match jump.
                if self.address_query.is_none() {
                    self.jump_to_match(start_idx);
                }
            }
        }
    }

    // ── Hex byte access ───────────────────────────────────────────────────────

    pub fn find_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let next = match self.search_match_index {
            Some(idx) => (idx + 1) % self.search_matches.len(),
            None => 0,
        };
        self.search_match_index = Some(next);
        self.jump_to_match(next);
    }

    pub fn find_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        let prev = match self.search_match_index {
            Some(idx) if idx > 0 => idx - 1,
            _ => self.search_matches.len() - 1,
        };
        self.search_match_index = Some(prev);
        self.jump_to_match(prev);
    }

    /// `n` key: forward in search direction, backward if search_forward=false.
    pub fn find_next_in_dir(&mut self) {
        if self.search_forward {
            self.find_next()
        } else {
            self.find_prev()
        }
    }

    /// `N` key: backward in search direction, forward if search_forward=false.
    pub fn find_prev_in_dir(&mut self) {
        if self.search_forward {
            self.find_prev()
        } else {
            self.find_next()
        }
    }

    pub fn jump_to_match(&mut self, match_idx: usize) {
        if let Some(&(line, byte_start, byte_end)) = self.search_matches.get(match_idx) {
            self.line_offset = line;
            if self.mode == ViewerMode::Hex {
                return;
            } // hex is fixed-width, no horiz scroll
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
        self.is_searching = false;
    }

    /// For hex mode: apply the address-jump part of the query immediately (no I/O).
    /// Call before dispatching the background pattern search.
    pub fn hex_apply_address_jump(&mut self, query: &str, file_size: usize) {
        let trimmed = query.trim();
        let addr_opt: Option<usize> = if let Some(rest) = trimmed
            .strip_prefix("0x")
            .or_else(|| trimmed.strip_prefix("0X"))
        {
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()) {
                usize::from_str_radix(rest, 16).ok()
            } else {
                None
            }
        } else if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
            let padded = if !trimmed.len().is_multiple_of(2) {
                format!("{}0", trimmed)
            } else {
                trimmed.to_string()
            };
            usize::from_str_radix(&padded, 16).ok()
        } else {
            None
        };
        if let Some(addr) = addr_opt {
            if addr < file_size {
                self.line_offset = addr / 16;
            }
            self.address_query = Some(query.to_string());
        }
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
    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_hexdigit()) {
            if let Ok(addr) = usize::from_str_radix(rest, 16) {
                return HexSearchPattern::Address(addr);
            }
        }
    }

    // Pure hex digits (no spaces): treat as address.
    // Odd-length queries are right-padded with '0' so "fff" aligns to the row
    // boundary 0xFFF0 (same row as "ffff") rather than the unaligned 0xFF0.
    let all_hex = !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_hexdigit());
    if all_hex {
        let padded = if !trimmed.len().is_multiple_of(2) {
            format!("{}0", trimmed)
        } else {
            trimmed.to_string()
        };
        if let Ok(addr) = usize::from_str_radix(&padded, 16) {
            return HexSearchPattern::Address(addr);
        }
    }

    // Hex digits with spaces: byte pattern (e.g. "4D 5A 90")
    let has_space = trimmed.contains(' ');
    let all_hex_space = trimmed.chars().all(|c| c.is_ascii_hexdigit() || c == ' ');
    if all_hex_space && has_space && !trimmed.is_empty() {
        let no_space: String = trimmed.chars().filter(|&c| c != ' ').collect();
        if no_space.len() >= 2 && no_space.len().is_multiple_of(2) {
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

/// Returns true if the hex-mode query requires a byte-pattern scan (background job needed).
/// Address-only queries with odd digit counts are instant jumps with no pattern.
pub fn hex_query_has_pattern(query: &str) -> bool {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return false;
    }
    // Explicit 0x address with even-length digits → also a byte pattern
    if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        if rest.chars().all(|c| c.is_ascii_hexdigit()) {
            return rest.len() >= 2 && rest.len() % 2 == 0;
        }
    }
    // Pure hex digits: even length = address + pattern, odd = address only
    if !trimmed.is_empty() && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        return trimmed.len() >= 2 && trimmed.len().is_multiple_of(2);
    }
    // Space-separated hex bytes or plain text → always needs a search
    true
}

/// Simple linear byte-pattern search. Returns (start, end) file offsets.
fn find_byte_pattern(haystack: &[u8], needle: &[u8]) -> Vec<(usize, usize)> {
    if needle.is_empty() {
        return vec![];
    }
    let mut result = Vec::new();
    let n = needle.len();
    if haystack.len() < n {
        return result;
    }
    for i in 0..=haystack.len() - n {
        if &haystack[i..i + n] == needle {
            result.push((i, i + n));
        }
    }
    result
}

/// Case-insensitive byte-pattern search. `lower_needle` must already be lowercased.
fn find_byte_pattern_ci(haystack: &[u8], lower_needle: &[u8]) -> Vec<(usize, usize)> {
    if lower_needle.is_empty() {
        return vec![];
    }
    let n = lower_needle.len();
    if haystack.len() < n {
        return vec![];
    }
    let mut result = Vec::new();
    for i in 0..=haystack.len() - n {
        if lower_needle
            .iter()
            .zip(&haystack[i..i + n])
            .all(|(&a, &b)| a == b.to_ascii_lowercase())
        {
            result.push((i, i + n));
        }
    }
    result
}

// ──────────────────────────────────────────────────────────────────────────────
// ViewerMode
// ──────────────────────────────────────────────────────────────────────────────

/// Viewer display mode: text or binary hexadecimal
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewerMode {
    Text,
    Hex,
}

// ──────────────────────────────────────────────────────────────────────────────
// TextEncoding
// ──────────────────────────────────────────────────────────────────────────────

/// Returns the largest prefix of `bytes` that ends on a valid UTF-8 character
/// boundary. Trims at most 3 trailing bytes (max continuation bytes in one
/// sequence). Used so a sample truncated at an arbitrary byte offset doesn't
/// cause a false UTF-8 validity failure.
fn trim_to_utf8_boundary(bytes: &[u8]) -> usize {
    let len = bytes.len();
    // Walk backward at most 3 bytes looking for a non-continuation byte.
    for back in 1..=3usize {
        if back > len {
            break;
        }
        let i = len - back;
        let b = bytes[i];
        if b & 0xC0 == 0x80 {
            continue;
        } // continuation byte, keep looking
          // b is ASCII or a lead byte. Check if the sequence it starts fits.
        let seq_len = if b < 0x80 {
            1
        } else if b & 0xE0 == 0xC0 {
            2
        } else if b & 0xF0 == 0xE0 {
            3
        } else if b & 0xF8 == 0xF0 {
            4
        } else {
            1
        }; // invalid — let from_utf8 reject it
        return if i + seq_len > len { i } else { len };
    }
    len
}

/// Text encoding for file viewing and decoding
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    ShiftJis,
    EucJp,
    Iso8859_1,
    Windows1252,
}

impl TextEncoding {
    /// Decode bytes using this encoding
    pub fn decode(&self, bytes: &[u8]) -> String {
        match self {
            TextEncoding::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
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
        if bytes.starts_with(b"\xFF\xFE") {
            return TextEncoding::Utf16Le;
        }
        if bytes.starts_with(b"\xFE\xFF") {
            return TextEncoding::Utf16Be;
        }
        let payload = if bytes.starts_with(b"\xEF\xBB\xBF") {
            &bytes[3..]
        } else {
            bytes
        };
        if bytes.starts_with(b"\xEF\xBB\xBF") {
            return TextEncoding::Utf8;
        }

        // Strict UTF-8 check: trim the tail to a valid boundary so a sample
        // truncated in the middle of a multi-byte sequence (e.g. Japanese UTF-8
        // cut at 16 KB) doesn't cause a false SJIS detection.
        let utf8_end = trim_to_utf8_boundary(payload);
        if std::str::from_utf8(&payload[..utf8_end]).is_ok() {
            return TextEncoding::Utf8;
        }

        // Statistical analysis for Japanese encodings on a 4 KB sample.
        let sample = &payload[..payload.len().min(4096)];
        let mut sjis: i32 = 0;
        let mut eucjp: i32 = 0;
        let mut i = 0;
        while i < sample.len() {
            let b = sample[i];
            // Shift-JIS lead bytes: 0x81–0x9F or 0xE0–0xFC
            if (0x81..=0x9F).contains(&b) || (0xE0..=0xFC).contains(&b) {
                if i + 1 < sample.len() {
                    let b2 = sample[i + 1];
                    if (0x40..=0x7E).contains(&b2) || (0x80..=0xFC).contains(&b2) {
                        sjis += 2;
                        i += 2;
                        continue;
                    }
                }
                sjis -= 1;
            }
            // EUC-JP lead bytes: 0xA1–0xFE (and SS2 0x8E, SS3 0x8F)
            if (0xA1..=0xFE).contains(&b) {
                if i + 1 < sample.len() && sample[i + 1] >= 0xA1 && sample[i + 1] <= 0xFE {
                    eucjp += 2;
                    i += 2;
                    continue;
                }
                eucjp -= 1;
            }
            if b == 0x8E && i + 1 < sample.len() && sample[i + 1] >= 0xA1 {
                // SS2 half-width kana
                eucjp += 1;
                i += 2;
                continue;
            }
            i += 1;
        }
        if sjis > 0 || eucjp > 0 {
            if sjis >= eucjp {
                TextEncoding::ShiftJis
            } else {
                TextEncoding::EucJp
            }
        } else {
            TextEncoding::Windows1252
        }
    }

    /// Cycle to the next encoding in the rotation
    pub fn next(&self) -> Self {
        match self {
            TextEncoding::Utf8 => TextEncoding::Utf16Le,
            TextEncoding::Utf16Le => TextEncoding::Utf16Be,
            TextEncoding::Utf16Be => TextEncoding::ShiftJis,
            TextEncoding::ShiftJis => TextEncoding::EucJp,
            TextEncoding::EucJp => TextEncoding::Iso8859_1,
            TextEncoding::Iso8859_1 => TextEncoding::Windows1252,
            TextEncoding::Windows1252 => TextEncoding::Utf8,
        }
    }

    /// Get the human-readable name of this encoding
    pub fn name(&self) -> &'static str {
        match self {
            TextEncoding::Utf8 => "UTF-8",
            TextEncoding::Utf16Le => "UTF-16 LE",
            TextEncoding::Utf16Be => "UTF-16 BE",
            TextEncoding::ShiftJis => "Shift-JIS",
            TextEncoding::EucJp => "EUC-JP",
            TextEncoding::Iso8859_1 => "ISO-8859-1",
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
                    // Branches for `b < 0x80` (ASCII byte) and `b < 0xC0` (orphan/invalid
                    // continuation byte) both yield seq_len 1, but represent distinct UTF-8
                    // cases per the spec; kept separate for clarity/correctness, not merged.
                    #[allow(clippy::if_same_then_else)]
                    let seq_len = if b < 0x80 {
                        1
                    } else if b < 0xC0 {
                        1
                    } else if b < 0xE0 {
                        2
                    } else if b < 0xF0 {
                        3
                    } else {
                        4
                    };
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
                    let is_lead = (0x81..=0x9F).contains(&b) || (0xE0..=0xFC).contains(&b);
                    if is_lead && i + 1 < bytes.len() {
                        let (s, _) =
                            encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if is_lead {
                        ('.', 1)
                    } else if (0xA1..=0xDF).contains(&b) {
                        let (s, _) =
                            encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes[i..i + 1]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 1)
                    } else {
                        (
                            if (32..=126).contains(&b) {
                                b as char
                            } else {
                                '\u{FFFD}'
                            },
                            1,
                        )
                    }
                }
                TextEncoding::EucJp => {
                    let b = bytes[i];
                    // The `(0xA1..=0xFE)` and `b == 0x8E` branches both decode a 2-byte
                    // EUC-JP sequence and look identical, but they match distinct lead-byte
                    // ranges (JIS X 0208 vs. half-width katakana via SS2); kept separate to
                    // mirror the EUC-JP spec rather than merged for brevity.
                    #[allow(clippy::if_same_then_else)]
                    if (0xA1..=0xFE).contains(&b) && i + 1 < bytes.len() {
                        let (s, _) =
                            encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if b == 0x8E && i + 1 < bytes.len() {
                        let (s, _) =
                            encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 2]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 2)
                    } else if b == 0x8F && i + 2 < bytes.len() {
                        let (s, _) =
                            encoding_rs::EUC_JP.decode_without_bom_handling(&bytes[i..i + 3]);
                        (s.chars().next().unwrap_or('\u{FFFD}'), 3)
                    } else {
                        (
                            if (32..=126).contains(&b) {
                                b as char
                            } else {
                                '\u{FFFD}'
                            },
                            1,
                        )
                    }
                }
                TextEncoding::Iso8859_1 | TextEncoding::Windows1252 => {
                    let (s, _) =
                        encoding_rs::WINDOWS_1252.decode_without_bom_handling(&bytes[i..i + 1]);
                    (s.chars().next().unwrap_or('\u{FFFD}'), 1)
                }
            };
            let display = if raw_ch.is_control() || raw_ch == '\u{FFFD}' {
                '.'
            } else {
                raw_ch
            };
            result.push((display, i, i + consumed));
            i += consumed;
        }
        result
    }
}

fn decode_utf16_le(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

fn decode_utf16_be(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect();
    String::from_utf16_lossy(&u16s)
}

// ──────────────────────────────────────────────────────────────────────────────
// Seekable byte-pattern search helper
// ──────────────────────────────────────────────────────────────────────────────

/// Chunk-based byte-pattern search for SeekableFile.
/// Reads the file in 4 MB chunks with a trailing overlap of `needle.len()-1` bytes
/// so that patterns spanning a chunk boundary are never missed.
fn search_seekable_bytes(
    seekable: &SeekableFile,
    needle: &[u8],
    case_insensitive: bool,
) -> Vec<(usize, usize)> {
    const CHUNK: usize = 4 * 1024 * 1024;
    let overlap = needle.len().saturating_sub(1);
    let file_size = seekable.size as usize;
    let mut results = Vec::new();
    if needle.is_empty() || file_size == 0 {
        return results;
    }

    let lower_needle: Vec<u8> = if case_insensitive {
        needle.iter().map(|b| b.to_ascii_lowercase()).collect()
    } else {
        vec![]
    };

    let mut chunk_start = 0usize;
    while chunk_start < file_size {
        // Re-read `overlap` bytes from end of previous chunk so cross-boundary
        // patterns are found in the combined window.
        let read_start = chunk_start.saturating_sub(overlap);
        let read_end = (chunk_start + CHUNK).min(file_size);
        let buf = match seekable.read_bytes(read_start as u64, read_end - read_start) {
            Ok(b) => b,
            Err(_) => break,
        };
        let hits = if case_insensitive {
            find_byte_pattern_ci(&buf, &lower_needle)
        } else {
            find_byte_pattern(&buf, needle)
        };
        for (s, e) in hits {
            let abs_start = read_start + s;
            let abs_end = read_start + e;
            // Include the match if it ends AFTER chunk_start.
            // Matches with abs_end <= chunk_start were entirely within the previous
            // iteration's read window and were already recorded there.
            // Cross-boundary matches (abs_start < chunk_start but abs_end > chunk_start)
            // could NOT have been found in the previous iteration (bytes past chunk_start
            // weren't in scope then), so they must be included here.
            if abs_end > chunk_start {
                results.push((abs_start, abs_end));
            }
        }
        chunk_start = read_end;
    }
    results
}

// ──────────────────────────────────────────────────────────────────────────────
// Tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn loc() -> Location {
        Location::Local(std::path::PathBuf::from("/test/file.txt"))
    }

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
    fn test_detect_utf8_truncated_at_multibyte_boundary() {
        // Simulate ROADMAP.md: valid UTF-8 with Japanese text whose sample
        // happens to be cut in the middle of a 3-byte sequence. Before the
        // fix this was misdetected as Shift-JIS.
        let text = "# ロードマップ\n".repeat(800); // ~16 KB of Japanese UTF-8
        let bytes = text.as_bytes();
        // Try every cut point in the last 4 bytes of the 16384-byte sample.
        for cut in 16380..=16384usize {
            let sample = &bytes[..cut.min(bytes.len())];
            assert_eq!(
                TextEncoding::detect(sample),
                TextEncoding::Utf8,
                "detect() returned non-UTF-8 for sample cut at {cut}"
            );
        }
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
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F, 0x72, 0x6C, 0x64, 0x21, 0x00, 0xFF,
            0xAA, 0x55, 0x01, 0x02, 0x03,
        ]);
        assert_eq!(v.hex_line_count(), 2);
        let (offset, bytes) = v.get_hex_bytes_vec(0).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(&bytes[0..4], &[0x48, 0x65, 0x6C, 0x6C]);

        let (offset, bytes) = v.get_hex_bytes_vec(1).unwrap();
        assert_eq!(offset, 16);
        assert_eq!(bytes, vec![0x01, 0x02, 0x03]);
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

    // ── SeekableFile tests ────────────────────────────────────────────────────

    #[test]
    fn test_seekable_file_read_bytes() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"Hello, World! This is test data.").unwrap();
        tmp.flush().unwrap();
        let size = std::fs::metadata(tmp.path()).unwrap().len();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), size);

        assert_eq!(sf.read_bytes(0, 5).unwrap(), b"Hello");
        assert_eq!(sf.read_bytes(7, 5).unwrap(), b"World");
        // Read past end: should return what's available
        assert_eq!(sf.read_bytes(size - 2, 100).unwrap().len(), 2);
        // Empty read
        assert_eq!(sf.read_bytes(0, 0).unwrap(), b"");
    }

    #[test]
    fn test_filebytes_seekable_len() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"1234567890").unwrap();
        tmp.flush().unwrap();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), 10);
        let fb = FileBytes::Seekable(sf);
        assert_eq!(fb.len(), 10);
        assert!(!fb.is_empty());
    }

    #[test]
    fn test_seekable_text_returns_none() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"hello").unwrap();
        tmp.flush().unwrap();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), 5);
        let buffer = ViewerBuffer::new(
            FileBytes::Seekable(sf),
            LineIndex {
                offsets: vec![0],
                is_complete: true,
            },
        );
        let mut vs = ViewerState::new(loc());
        vs.buffer = Some(buffer);
        assert_eq!(vs.text(), None);
    }

    #[test]
    fn test_get_line_bytes_seekable() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"Line one\nLine two\nLine three\n").unwrap();
        tmp.flush().unwrap();
        let size = std::fs::metadata(tmp.path()).unwrap().len();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), size);
        let buffer = ViewerBuffer::new(
            FileBytes::Seekable(sf),
            LineIndex {
                offsets: vec![0, 9, 18],
                is_complete: true,
            },
        );
        let mut vs = ViewerState::new(loc());
        vs.buffer = Some(buffer);

        assert_eq!(vs.get_line_bytes(0), Some(b"Line one".to_vec()));
        assert_eq!(vs.get_line_bytes(1), Some(b"Line two".to_vec()));
        assert_eq!(vs.get_line_bytes(2), Some(b"Line three".to_vec()));
        assert_eq!(vs.get_line_bytes(3), None);
    }

    #[test]
    fn test_get_hex_bytes_vec_seekable() {
        use std::io::Write;
        let data: Vec<u8> = (0u8..32).collect();
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(&data).unwrap();
        tmp.flush().unwrap();
        let size = std::fs::metadata(tmp.path()).unwrap().len();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), size);
        let buffer = ViewerBuffer::new(FileBytes::Seekable(sf), LineIndex::new_complete_empty());
        let mut vs = ViewerState::new(loc());
        vs.buffer = Some(buffer);

        let (offset, bytes) = vs.get_hex_bytes_vec(0).unwrap();
        assert_eq!(offset, 0);
        assert_eq!(bytes, (0u8..16).collect::<Vec<_>>());

        let (offset, bytes) = vs.get_hex_bytes_vec(1).unwrap();
        assert_eq!(offset, 16);
        assert_eq!(bytes, (16u8..32).collect::<Vec<_>>());

        assert!(vs.get_hex_bytes_vec(2).is_none());
    }

    #[test]
    fn test_search_on_seekable_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(b"Hello World\nHello Rust\nGoodbye World\n")
            .unwrap();
        tmp.flush().unwrap();
        let size = std::fs::metadata(tmp.path()).unwrap().len();
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), size);
        let buffer = ViewerBuffer::new(
            FileBytes::Seekable(sf),
            LineIndex {
                offsets: vec![0, 12, 23],
                is_complete: true,
            },
        );
        let mut vs = ViewerState::new(loc());
        vs.buffer = Some(buffer);

        vs.start_search("Hello".to_string(), None);
        assert_eq!(vs.search_matches.len(), 2);
        assert_eq!(vs.search_match_index, Some(0));
        vs.find_next();
        assert_eq!(vs.search_match_index, Some(1));
    }

    #[test]
    fn test_search_seekable_bytes_chunked() {
        use std::io::Write;
        // Pattern spanning chunk boundary: write 4MB+4 bytes; needle at boundary.
        let chunk = 4 * 1024 * 1024;
        let mut data = vec![0xAAu8; chunk];
        // Place needle 0xDE 0xAD 0xBE 0xEF crossing the boundary (last 2 bytes of first chunk,
        // first 2 of second chunk).
        data[chunk - 2] = 0xDE;
        data[chunk - 1] = 0xAD;
        data.extend_from_slice(&[0xBE, 0xEF]);
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(&data).unwrap();
        tmp.flush().unwrap();
        let size = data.len() as u64;
        let sf = SeekableFile::new(std::fs::File::open(tmp.path()).unwrap(), size);
        let needle = [0xDE, 0xAD, 0xBE, 0xEF];
        let hits = search_seekable_bytes(&sf, &needle, false);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, chunk - 2);
        assert_eq!(hits[0].1, chunk + 2);
    }
}
