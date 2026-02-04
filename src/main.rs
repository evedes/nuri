mod cli;
mod color;
mod pipeline;
mod preview;
mod theme;
mod tui;

use anyhow::Result;
use clap::Parser;

use cli::Args;
use pipeline::assign::assign_slots;
use pipeline::contrast::enforce_contrast;
use pipeline::detect::detect_mode;
use pipeline::extract::{extract_colors, load_and_prepare};
use theme::GhosttyTheme;

fn main() -> Result<()> {
    let args = Args::parse();

    // 1. Load and prepare image pixels
    let pixels = load_and_prepare(&args.image)?;

    // 2. Extract dominant colors via K-means
    let colors = extract_colors(&pixels, args.colors);

    // 3. Detect dark/light mode (respect --mode override)
    let mode = args.mode.unwrap_or_else(|| detect_mode(&pixels));

    // 4. Assign colors to ANSI palette slots
    let mut palette = assign_slots(&colors, mode);

    // 5. Enforce WCAG contrast minimums
    enforce_contrast(&mut palette);

    // 6. Build theme
    let theme = GhosttyTheme::from_palette(palette);

    // 7. Preview (can combine with other output modes)
    if args.preview {
        preview::print_preview(&theme.palette);
    }

    // 8. Output
    let name = args.name.unwrap_or_else(|| default_theme_name(&args.image));

    if args.install {
        theme.install(&name)?;
        eprintln!("Installed theme '{name}' to ~/.config/ghostty/themes/{name}");
    } else if let Some(ref path) = args.output {
        theme.write_to(path)?;
        eprintln!("Wrote theme to {}", path.display());
    } else {
        print!("{}", theme.serialize());
    }

    Ok(())
}

/// Derive a theme name from the image filename stem.
fn default_theme_name(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("theme")
        .to_string()
}
