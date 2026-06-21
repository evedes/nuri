use anyhow::Result;
use clap::Parser;

use nuri::cli::AccentVariant;
use nuri::cli::Args;
use nuri::pipeline::assign::assign_slots_with_accent;
use nuri::pipeline::contrast::enforce_contrast;
use nuri::pipeline::detect::detect_mode;
use nuri::pipeline::extract::{extract_colors, load_and_prepare};
use nuri::tui;

fn main() -> Result<()> {
    let args = Args::parse();

    // Validate --min-contrast
    let min_contrast = validate_min_contrast(args.min_contrast);

    // 1. Load and prepare image pixels
    let pixels = load_and_prepare(&args.image)?;

    // 2. Extract dominant colors via K-means
    let colors = extract_colors(&pixels, args.colors);

    // 3. Detect dark/light mode (respect --mode override)
    let mode = args.mode.unwrap_or_else(|| detect_mode(&pixels));

    // 4. Assign colors to ANSI palette slots
    let mut palette = assign_slots_with_accent(&colors, mode, args.accent, AccentVariant::Vibrant);

    // 5. Enforce WCAG contrast minimums
    enforce_contrast(&mut palette, min_contrast);

    // 6. Derive theme name
    let name = args.name.unwrap_or_else(|| default_theme_name(&args.image));

    // 6b. Non-interactive escape hatch: print the resolved palette and exit.
    if let Some(format) = args.format {
        print!("{}", nuri::output::render(&palette, &name, mode, format));
        return Ok(());
    }

    // 7. Launch interactive TUI
    let targets = args.target.clone();
    let mut tui_app =
        tui::TuiApp::new(palette, colors, args.image, mode, name, pixels, args.colors);
    tui_app.set_targets(targets);
    tui_app.set_accent(args.accent);
    tui::run(tui_app)
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
