use std::path::Path;

use anyhow::Result;

use crate::pipeline::assign::AnsiPalette;

/// A serializable Ghostty terminal theme.
#[derive(Debug, Clone)]
pub struct GhosttyTheme {
    pub palette: AnsiPalette,
}

impl GhosttyTheme {
    /// Create a theme from an assigned palette.
    pub fn from_palette(palette: AnsiPalette) -> Self {
        Self { palette }
    }

    /// Serialize the theme to the Ghostty key-value format.
    pub fn serialize(&self) -> String {
        todo!("Ticket 8: theme serialization")
    }

    /// Install the theme to ~/.config/ghostty/themes/<name>.
    pub fn install(&self, _name: &str) -> Result<()> {
        todo!("Ticket 8: theme install")
    }

    /// Write the theme to an arbitrary path.
    pub fn write_to(&self, _path: &Path) -> Result<()> {
        todo!("Ticket 8: theme write_to")
    }
}
