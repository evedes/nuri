use crate::cli::ThemeMode;
use crate::pipeline::extract::ExtractedColor;

/// The full ANSI palette plus special Ghostty theme colors.
#[derive(Debug, Clone)]
pub struct AnsiPalette {
    /// ANSI colors 0-15.
    pub slots: [palette::Srgb<u8>; 16],
    pub background: palette::Srgb<u8>,
    pub foreground: palette::Srgb<u8>,
    pub cursor_color: palette::Srgb<u8>,
    pub cursor_text: palette::Srgb<u8>,
    pub selection_bg: palette::Srgb<u8>,
    pub selection_fg: palette::Srgb<u8>,
}

/// Map extracted colors to the 16 ANSI palette slots plus special colors.
pub fn assign_slots(_colors: &[ExtractedColor], _mode: ThemeMode) -> AnsiPalette {
    todo!("Ticket 6: hue-based slot assignment")
}
