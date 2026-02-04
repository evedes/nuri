pub mod widgets;

use anyhow::Result;

use crate::cli::ThemeMode;
use crate::pipeline::assign::AnsiPalette;
use crate::pipeline::extract::ExtractedColor;
use std::path::PathBuf;

/// State for the interactive TUI application.
#[allow(dead_code)]
pub struct TuiApp {
    pub palette: AnsiPalette,
    pub extracted_colors: Vec<ExtractedColor>,
    pub image_path: PathBuf,
    pub mode: ThemeMode,
    pub selected_slot: Option<usize>,
    pub theme_name: String,
}

/// Launch the TUI application.
pub fn run(_app: TuiApp) -> Result<()> {
    todo!("Ticket 11: TUI app shell and event loop")
}
