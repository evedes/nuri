use std::path::PathBuf;

use clap::Parser;

use crate::backends::Target;

/// Generate color themes from wallpaper images.
#[derive(Parser, Debug)]
#[command(name = "nuri", version, about)]
pub struct Args {
    /// Path to the input image
    pub image: PathBuf,

    /// Theme name (defaults to image filename stem)
    #[arg(short, long)]
    pub name: Option<String>,

    /// Force dark or light mode (auto-detected if omitted)
    #[arg(short, long, value_enum)]
    pub mode: Option<ThemeMode>,

    /// Target theme format(s), comma-separated (e.g. ghostty,zellij)
    #[arg(short = 't', long, value_enum, value_delimiter = ',')]
    pub target: Vec<Target>,

    /// Number of K-means clusters
    #[arg(short = 'k', long = "colors", default_value_t = 16)]
    pub colors: usize,

    /// Minimum accent contrast ratio against background
    #[arg(long, default_value_t = 4.5)]
    pub min_contrast: f32,

    /// Generate a monochromatic palette using shades of a single accent color
    #[arg(long, value_enum)]
    pub accent: Option<AccentColor>,

    /// Print the resolved palette to stdout in this format and exit (skips the TUI).
    /// Target-agnostic — emits the raw palette so external tools can template any app.
    #[arg(long, value_enum)]
    pub format: Option<OutputFormat>,
}

/// Non-interactive output formats for the resolved palette (the "escape hatch").
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    /// JSON object: theme name, mode, special colors, and the 16 ANSI slots.
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ThemeMode {
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum AccentColor {
    Blue,
    Green,
    Yellow,
    Red,
    Purple,
    Gray,
}

/// Style variants for monochromatic accent palettes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccentVariant {
    Vibrant,
    Muted,
    Dark,
    Pastel,
}

impl AccentVariant {
    pub const ALL: [AccentVariant; 4] = [
        AccentVariant::Vibrant,
        AccentVariant::Muted,
        AccentVariant::Dark,
        AccentVariant::Pastel,
    ];

    pub fn label(self) -> &'static str {
        match self {
            AccentVariant::Vibrant => "Vibrant",
            AccentVariant::Muted => "Muted",
            AccentVariant::Dark => "Dark",
            AccentVariant::Pastel => "Pastel",
        }
    }

    /// (lightness_offset, chroma_scale) applied to base monochromatic params.
    pub fn modifiers(self) -> (f32, f32) {
        match self {
            AccentVariant::Vibrant => (0.0, 1.3),
            AccentVariant::Muted => (0.05, 0.6),
            AccentVariant::Dark => (-0.10, 1.0),
            AccentVariant::Pastel => (0.10, 0.5),
        }
    }
}
