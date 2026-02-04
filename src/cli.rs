use std::path::PathBuf;

use clap::Parser;

/// Generate Ghostty terminal color themes from wallpaper images.
#[derive(Parser, Debug)]
#[command(name = "ghostty-themer", version, about)]
pub struct Args {
    /// Path to the input image
    pub image: PathBuf,

    /// Theme name (defaults to image filename stem)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Force dark or light mode (auto-detected if omitted)
    #[arg(short, long, value_enum)]
    pub mode: Option<ThemeMode>,

    /// Write theme to this file instead of stdout
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Install theme to ~/.config/ghostty/themes/
    #[arg(long, conflicts_with = "output")]
    pub install: bool,

    /// Print a colored terminal preview of the palette
    #[arg(long)]
    pub preview: bool,

    /// Launch interactive TUI mode
    #[arg(long)]
    pub tui: bool,

    /// Number of K-means clusters
    #[arg(short = 'k', long = "colors", default_value_t = 16)]
    pub colors: usize,

    /// Minimum accent contrast ratio against background
    #[arg(long, default_value_t = 4.5)]
    pub min_contrast: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ThemeMode {
    Dark,
    Light,
}
