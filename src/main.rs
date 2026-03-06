use anyhow::{bail, Result};
use clap::Parser;

use nuri::backends::{get_backend, ghostty, Target, ThemeBackend};
use nuri::cli::AccentVariant;
use nuri::cli::Args;
use nuri::pipeline::assign::assign_slots_with_accent;
use nuri::pipeline::contrast::enforce_contrast;
use nuri::pipeline::detect::detect_mode;
use nuri::pipeline::extract::{extract_colors, load_and_prepare};
use nuri::{preview, tui};

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
    let mut palette = assign_slots_with_accent(&colors, mode, args.accent, AccentVariant::Vibrant);

    // 5. Enforce WCAG contrast minimums
    enforce_contrast(&mut palette, min_contrast);

    // 6. Derive theme name
    let name = args.name.unwrap_or_else(|| default_theme_name(&args.image));

    // 7. TUI mode: launch interactive editor
    if args.tui {
        let targets = args.target.clone();
        let mut tui_app =
            tui::TuiApp::new(palette, colors, args.image, mode, name, pixels, args.colors);
        tui_app.set_targets(targets);
        tui_app.set_accent(args.accent);
        return tui::run(tui_app);
    }

    // 8. CLI mode: build theme and output
    // Default to Ghostty when no --target specified in CLI mode
    let targets = if args.target.is_empty() {
        vec![Target::Ghostty]
    } else {
        args.target.clone()
    };
    let backends: Vec<Box<dyn ThemeBackend>> = targets.iter().map(|t| get_backend(*t)).collect();

    if args.preview {
        preview::print_preview(&palette);
    }

    if args.install {
        // Check --no-clobber for Ghostty targets
        if args.no_clobber && targets.contains(&Target::Ghostty) {
            let theme_path = ghostty::theme_path(&name)?;
            if theme_path.exists() {
                bail!(
                    "theme '{}' already exists at {}. Remove it first or omit --no-clobber.",
                    name,
                    theme_path.display()
                );
            }
        }
        for backend in &backends {
            let installed_path = backend.install(&palette, &name)?;
            eprintln!(
                "Installed {} theme '{name}' to {}",
                backend.name(),
                installed_path.display()
            );
        }
    } else if let Some(ref path) = args.output {
        if backends.len() > 1 {
            bail!("cannot use --output with multiple targets; use --install instead");
        }
        backends[0].write_to(&palette, &name, path)?;
        eprintln!("Wrote theme to {}", path.display());
    } else {
        if backends.len() > 1 {
            bail!(
                "cannot output multiple targets to stdout; use --install or specify a single --target"
            );
        }
        print!("{}", backends[0].serialize(&palette, &name));
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
