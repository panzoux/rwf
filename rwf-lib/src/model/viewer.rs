//! File viewer state and operations
//!
//! This module defines the viewer state for displaying file contents
//! in text or hex mode with encoding support.

use crate::model::Location;

/// Viewer state for displaying file contents
#[derive(Debug, Clone)]
pub struct ViewerState {
    /// The file being viewed
    pub location: Location,
    /// Current viewer mode (text or hex)
    pub mode: ViewerMode,
    /// File contents as bytes
    pub contents: Vec<u8>,
    /// Current encoding for text mode
    pub encoding: TextEncoding,
    /// Decoded text content (cached)
    pub decoded_text: Option<String>,
    /// Current line offset (for scrolling)
    pub line_offset: usize,
    /// Current column offset (for horizontal scrolling)
    pub column_offset: usize,
    /// Search query in viewer
    pub search_query: Option<String>,
    /// Current search match index
    pub search_match_index: Option<usize>,
    /// All search match positions (line, column)
    pub search_matches: Vec<(usize, usize)>,
}

impl ViewerState {
    /// Create a new viewer state for a file
    pub fn new(location: Location) -> Self {
        Self {
            location,
            mode: ViewerMode::Text,
            contents: Vec::new(),
            encoding: TextEncoding::Utf8,
            decoded_text: None,
            line_offset: 0,
            column_offset: 0,
            search_query: None,
            search_match_index: None,
            search_matches: Vec::new(),
        }
    }

    /// Set the file contents
    pub fn set_contents(&mut self, contents: Vec<u8>) {
        self.contents = contents;
        self.decoded_text = None; // Invalidate cache
        self.decode_text();
    }

    /// Decode text content with current encoding
    pub fn decode_text(&mut self) {
        if self.mode != ViewerMode::Text {
            return;
        }

        self.decoded_text = Some(self.encoding.decode(&self.contents));
    }

    /// Cycle to the next encoding
    pub fn cycle_encoding(&mut self) {
        self.encoding = self.encoding.next();
        self.decode_text();
    }

    /// Get the decoded text content
    pub fn text(&self) -> Option<&str> {
        self.decoded_text.as_deref()
    }

    /// Get the number of lines in the text
    pub fn line_count(&self) -> usize {
        self.text()
            .map(|t| t.lines().count())
            .unwrap_or(0)
    }

    /// Get the number of lines in hex mode (16 bytes per line)
    pub fn hex_line_count(&self) -> usize {
        self.contents.len().div_ceil(16)
    }

    /// Get a hex line at the given index
    /// Returns (offset, hex_bytes, ascii_repr)
    pub fn get_hex_line(&self, line_idx: usize) -> Option<(usize, String, String)> {
        let offset = line_idx * 16;
        if offset >= self.contents.len() {
            return None;
        }

        let end = (offset + 16).min(self.contents.len());
        let bytes = &self.contents[offset..end];

        // Format hex bytes
        let mut hex_str = String::new();
        for (i, byte) in bytes.iter().enumerate() {
            if i > 0 && i % 8 == 0 {
                hex_str.push(' '); // Extra space after 8 bytes
            }
            hex_str.push_str(&format!("{:02X} ", byte));
        }

        // Pad hex string if less than 16 bytes
        let padding_needed = (16 - bytes.len()) * 3 + if bytes.len() <= 8 { 1 } else { 0 };
        hex_str.push_str(&" ".repeat(padding_needed));

        // Format ASCII representation
        let ascii_str: String = bytes.iter()
            .map(|&b| {
                if (32..=126).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();

        Some((offset, hex_str, ascii_str))
    }

    /// Move to the start of the current line
    pub fn move_to_line_start(&mut self) {
        self.column_offset = 0;
    }

    /// Move to the end of the current line
    pub fn move_to_line_end(&mut self, viewport_width: usize) {
        if let Some(text) = self.text() {
            let lines: Vec<&str> = text.lines().collect();
            if let Some(line) = lines.get(self.line_offset) {
                let line_len = line.len();
                if line_len > viewport_width {
                    self.column_offset = line_len.saturating_sub(viewport_width);
                } else {
                    self.column_offset = 0;
                }
            }
        }
    }

    /// Jump to the top of the file
    pub fn jump_to_top(&mut self) {
        self.line_offset = 0;
        self.column_offset = 0;
    }

    /// Jump to the bottom of the file
    pub fn jump_to_bottom(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset = line_count.saturating_sub(viewport_height);
        self.column_offset = 0;
    }

    /// Scroll down by one line
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

    /// Scroll up by one line
    pub fn scroll_up(&mut self) {
        if self.line_offset > 0 {
            self.line_offset -= 1;
        }
    }

    /// Scroll down by one page
    pub fn page_down(&mut self, viewport_height: usize) {
        let line_count = if self.mode == ViewerMode::Hex {
            self.hex_line_count()
        } else {
            self.line_count()
        };
        self.line_offset = (self.line_offset + viewport_height).min(line_count.saturating_sub(viewport_height));
    }

    /// Scroll up by one page
    pub fn page_up(&mut self, viewport_height: usize) {
        self.line_offset = self.line_offset.saturating_sub(viewport_height);
    }

    /// Start a search
    pub fn start_search(&mut self, query: String) {
        self.search_query = Some(query.clone());
        self.search_matches.clear();
        self.search_match_index = None;

        // Find all matches
        if let Some(text) = self.decoded_text.as_ref() {
            for (line_idx, line) in text.lines().enumerate() {
                let mut start = 0;
                while let Some(pos) = line[start..].find(&query) {
                    self.search_matches.push((line_idx, start + pos));
                    start += pos + 1;
                }
            }
        }

        // Jump to first match
        if !self.search_matches.is_empty() {
            self.search_match_index = Some(0);
            self.jump_to_match(0);
        }
    }

    /// Find next search match
    pub fn find_next(&mut self) {
        if let Some(current_idx) = self.search_match_index {
            if current_idx + 1 < self.search_matches.len() {
                let next_idx = current_idx + 1;
                self.search_match_index = Some(next_idx);
                self.jump_to_match(next_idx);
            }
        }
    }

    /// Find previous search match
    pub fn find_prev(&mut self) {
        if let Some(current_idx) = self.search_match_index {
            if current_idx > 0 {
                let prev_idx = current_idx - 1;
                self.search_match_index = Some(prev_idx);
                self.jump_to_match(prev_idx);
            }
        }
    }

    /// Jump to a specific search match
    fn jump_to_match(&mut self, match_idx: usize) {
        if let Some(&(line, col)) = self.search_matches.get(match_idx) {
            self.line_offset = line;
            self.column_offset = col;
        }
    }

    /// Clear search
    pub fn clear_search(&mut self) {
        self.search_query = None;
        self.search_matches.clear();
        self.search_match_index = None;
    }
}

/// Viewer display mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewerMode {
    /// Text mode with encoding support
    Text,
    /// Hexadecimal mode
    Hex,
}

/// Text encoding options
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
    /// Decode bytes with this encoding
    pub fn decode(&self, bytes: &[u8]) -> String {
        match self {
            TextEncoding::Utf8 => {
                String::from_utf8_lossy(bytes).into_owned()
            }
            TextEncoding::Utf16Le => {
                decode_utf16_le(bytes)
            }
            TextEncoding::Utf16Be => {
                decode_utf16_be(bytes)
            }
            TextEncoding::ShiftJis => {
                // For now, fallback to UTF-8
                // TODO: Implement proper Shift-JIS decoding
                String::from_utf8_lossy(bytes).into_owned()
            }
            TextEncoding::EucJp => {
                // For now, fallback to UTF-8
                // TODO: Implement proper EUC-JP decoding
                String::from_utf8_lossy(bytes).into_owned()
            }
            TextEncoding::Iso8859_1 => {
                // ISO-8859-1 is a simple byte-to-char mapping
                bytes.iter().map(|&b| b as char).collect()
            }
            TextEncoding::Windows1252 => {
                // For now, treat as ISO-8859-1
                // TODO: Implement proper Windows-1252 decoding
                bytes.iter().map(|&b| b as char).collect()
            }
        }
    }

    /// Get the next encoding in the cycle
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

    /// Get the encoding name for display
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
}

/// Decode UTF-16 Little Endian
fn decode_utf16_le(bytes: &[u8]) -> String {
    let mut u16_vec = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        u16_vec.push(value);
    }
    String::from_utf16_lossy(&u16_vec)
}

/// Decode UTF-16 Big Endian
fn decode_utf16_be(bytes: &[u8]) -> String {
    let mut u16_vec = Vec::new();
    for chunk in bytes.chunks_exact(2) {
        let value = u16::from_be_bytes([chunk[0], chunk[1]]);
        u16_vec.push(value);
    }
    String::from_utf16_lossy(&u16_vec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_viewer_state_creation() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.txt"));
        let viewer = ViewerState::new(location.clone());
        
        assert_eq!(viewer.location, location);
        assert_eq!(viewer.mode, ViewerMode::Text);
        assert_eq!(viewer.encoding, TextEncoding::Utf8);
        assert_eq!(viewer.line_offset, 0);
        assert_eq!(viewer.column_offset, 0);
    }

    #[test]
    fn test_set_contents_and_decode() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.txt"));
        let mut viewer = ViewerState::new(location);
        
        let contents = b"Hello\nWorld\n".to_vec();
        viewer.set_contents(contents);
        
        assert_eq!(viewer.text(), Some("Hello\nWorld\n"));
        assert_eq!(viewer.line_count(), 2); // Two lines: "Hello" and "World"
    }

    #[test]
    fn test_encoding_cycle() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.txt"));
        let mut viewer = ViewerState::new(location);
        
        assert_eq!(viewer.encoding, TextEncoding::Utf8);
        viewer.cycle_encoding();
        assert_eq!(viewer.encoding, TextEncoding::Utf16Le);
        viewer.cycle_encoding();
        assert_eq!(viewer.encoding, TextEncoding::Utf16Be);
    }

    #[test]
    fn test_navigation() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.txt"));
        let mut viewer = ViewerState::new(location);
        
        let contents = b"Line 1\nLine 2\nLine 3\nLine 4\nLine 5".to_vec();
        viewer.set_contents(contents);
        
        // Test jump to top
        viewer.line_offset = 3;
        viewer.jump_to_top();
        assert_eq!(viewer.line_offset, 0);
        
        // Test jump to bottom
        viewer.jump_to_bottom(2);
        assert!(viewer.line_offset > 0);
        
        // Test scroll
        viewer.line_offset = 2;
        viewer.scroll_down(10);
        assert_eq!(viewer.line_offset, 2); // Can't scroll past end
        
        viewer.scroll_up();
        assert_eq!(viewer.line_offset, 1);
    }

    #[test]
    fn test_search() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.txt"));
        let mut viewer = ViewerState::new(location);
        
        let contents = b"Hello World\nHello Rust\nGoodbye World\n".to_vec();
        viewer.set_contents(contents);
        
        viewer.start_search("Hello".to_string());
        assert_eq!(viewer.search_matches.len(), 2);
        assert_eq!(viewer.search_match_index, Some(0));
        
        viewer.find_next();
        assert_eq!(viewer.search_match_index, Some(1));
        
        viewer.find_prev();
        assert_eq!(viewer.search_match_index, Some(0));
    }

    #[test]
    fn test_utf8_decoding() {
        let encoding = TextEncoding::Utf8;
        let bytes = "Hello 世界".as_bytes();
        let decoded = encoding.decode(bytes);
        assert_eq!(decoded, "Hello 世界");
    }

    #[test]
    fn test_iso8859_1_decoding() {
        let encoding = TextEncoding::Iso8859_1;
        let bytes = b"Hello \xE9"; // é in ISO-8859-1
        let decoded = encoding.decode(bytes);
        assert_eq!(decoded, "Hello é");
    }

    #[test]
    fn test_hex_viewer() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.bin"));
        let mut viewer = ViewerState::new(location);
        viewer.mode = ViewerMode::Hex;
        
        // Test with some binary data
        let contents = vec![
            0x48, 0x65, 0x6C, 0x6C, 0x6F, 0x20, 0x57, 0x6F, // "Hello Wo"
            0x72, 0x6C, 0x64, 0x21, 0x00, 0xFF, 0xAA, 0x55, // "rld!...."
            0x01, 0x02, 0x03, // Extra bytes
        ];
        viewer.set_contents(contents);
        
        assert_eq!(viewer.hex_line_count(), 2); // 19 bytes = 2 lines (16 + 3)
        
        // Test first line
        let (offset, hex, ascii) = viewer.get_hex_line(0).unwrap();
        assert_eq!(offset, 0);
        assert!(hex.contains("48 65 6C 6C"));
        assert!(ascii.contains("Hello"));
        
        // Test second line
        let (offset, hex, _ascii) = viewer.get_hex_line(1).unwrap();
        assert_eq!(offset, 16);
        assert!(hex.contains("01 02 03"));
    }

    #[test]
    fn test_hex_navigation() {
        let location = Location::Local(std::path::PathBuf::from("/test/file.bin"));
        let mut viewer = ViewerState::new(location);
        viewer.mode = ViewerMode::Hex;
        
        // Create 100 bytes of data (7 lines in hex mode)
        let contents = vec![0u8; 100];
        viewer.set_contents(contents);
        
        assert_eq!(viewer.hex_line_count(), 7);
        
        // Test scrolling
        viewer.scroll_down(5);
        assert_eq!(viewer.line_offset, 1);
        
        viewer.jump_to_bottom(5);
        assert_eq!(viewer.line_offset, 2); // 7 - 5 = 2
    }
}
