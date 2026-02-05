mod cli;
mod color;
mod pipeline;
mod preview;
mod theme;
mod tui;

use anyhow::{bail, Result};
use clap::Parser;

use cli::Args;
use pipeline::assign::assign_slots;
use pipeline::contrast::enforce_contrast;
use pipeline::detect::detect_mode;
use pipeline::extract::{extract_colors, load_and_prepare};
use theme::GhosttyTheme;

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate --min-contrast
    let min_contrast = validate_min_contrast(args.min_contrast);

    // 1. Load and prepare image pixels
    let pixels = load_and_prepare(&args.image)?;

    // Warn on tiny images
    if pixels.len() < 16 {
        eprintln!(
            "warning: very small image ({} pixels). Theme quality may be limited.",
            pixels.len()
        );
    }

    // 2. Extract dominant colors via K-means
    let colors = extract_colors(&pixels, args.colors);

    // Warn on few extracted colors
    if colors.len() < 6 {
        eprintln!(
            "warning: only {} distinct colors extracted (expected ≥ 6). \
             Some palette slots will be synthesized.",
            colors.len()
        );
    }

    // 3. Detect dark/light mode (respect --mode override)
    let mode = args.mode.unwrap_or_else(|| detect_mode(&pixels));

    // 4. Assign colors to ANSI palette slots
    let mut palette = assign_slots(&colors, mode);

    // 5. Enforce WCAG contrast minimums
    enforce_contrast(&mut palette, min_contrast);

    // 6. Derive theme name
    let name = args.name.unwrap_or_else(|| default_theme_name(&args.image));

    // 7. TUI mode: launch interactive editor
    if args.tui {
        let tui_app =
            tui::TuiApp::new(palette, colors, args.image, mode, name, pixels, args.colors);
        return tui::run(tui_app);
    }

    // 8. CLI mode: build theme and output
    let theme = GhosttyTheme::from_palette(palette);

    if args.preview {
        preview::print_preview(&theme.palette);
    }

    if args.install {
        // Check --no-clobber
        if args.no_clobber {
            let theme_path = GhosttyTheme::theme_path(&name)?;
            if theme_path.exists() {
                bail!(
                    "theme '{}' already exists at {}. Remove it first or omit --no-clobber.",
                    name,
                    theme_path.display()
                );
            }
        }
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

/// Validate and clamp --min-contrast to [1.0, 21.0].
fn validate_min_contrast(value: f32) -> f32 {
    if value < 1.0 {
        eprintln!("warning: --min-contrast {value} is below 1.0, clamping to 1.0");
        1.0
    } else if value > 21.0 {
        eprintln!("warning: --min-contrast {value} exceeds 21.0, clamping to 21.0");
        21.0
    } else {
        value
    }
}

/// Derive a theme name from the image filename stem.
fn default_theme_name(path: &std::path::Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("theme")
        .to_string()
}
