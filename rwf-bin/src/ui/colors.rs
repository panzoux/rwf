//! Color parsing utilities
//!
//! Converts color strings from config into ratatui Color types

use ratatui::style::Color;

/// Parse a color string from config into a ratatui Color
pub fn parse_color(color_str: &str) -> Color {
    match color_str {
        "Black" => Color::Black,
        "Red" => Color::Red,
        "Green" => Color::Green,
        "Yellow" => Color::Yellow,
        "Blue" => Color::Blue,
        "Magenta" => Color::Magenta,
        "Cyan" => Color::Cyan,
        "Gray" | "Grey" => Color::Gray,
        "DarkGray" | "DarkGrey" => Color::DarkGray,
        "LightRed" | "BrightRed" => Color::LightRed,
        "LightGreen" | "BrightGreen" => Color::LightGreen,
        "LightYellow" | "BrightYellow" => Color::LightYellow,
        "LightBlue" | "BrightBlue" => Color::LightBlue,
        "LightMagenta" | "BrightMagenta" => Color::LightMagenta,
        "LightCyan" | "BrightCyan" => Color::LightCyan,
        "White" => Color::White,
        _ => Color::White, // Default fallback
    }
}
