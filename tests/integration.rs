use std::path::{Path, PathBuf};
use std::process::Command;

use nuri::backends::ghostty::GhosttyBackend;
use nuri::backends::ThemeBackend;
use nuri::cli::ThemeMode;
use nuri::color::Color;
use nuri::pipeline::assign::assign_slots;
use nuri::pipeline::contrast::{enforce_contrast, DEFAULT_ACCENT_CONTRAST};
use nuri::pipeline::detect::detect_mode;
use nuri::pipeline::extract::{extract_colors, load_and_prepare};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("snapshots")
}

fn create_dark_photo(path: &Path) {
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        let r = ((x * 40) / 64) as u8;
        let g = ((y * 30) / 64) as u8 + 5;
        let b = 20 + ((x + y) % 15) as u8;
        image::Rgb([r, g, b])
    });
    img.save(path).unwrap();
}

fn create_light_photo(path: &Path) {
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        let r = 200 + ((x * 55) / 64) as u8;
        let g = 190 + ((y * 55) / 64) as u8;
        let b = 180 + (((x + y) * 30) / 128).min(75) as u8;
        image::Rgb([r, g, b])
    });
    img.save(path).unwrap();
}

fn create_monochrome(path: &Path) {
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        let v = ((x * 255) / 64 + (y * 255) / 64) as u8 / 2;
        image::Rgb([v, v, v])
    });
    img.save(path).unwrap();
}

fn create_colorful(path: &Path) {
    let img = image::RgbImage::from_fn(64, 64, |x, y| {
        let region = (x / 16) + (y / 16) * 4;
        match region % 8 {
            0 => image::Rgb([220, 50, 50]),   // red
            1 => image::Rgb([50, 200, 50]),   // green
            2 => image::Rgb([50, 50, 220]),   // blue
            3 => image::Rgb([220, 220, 50]),  // yellow
            4 => image::Rgb([200, 50, 200]),  // magenta
            5 => image::Rgb([50, 200, 200]),  // cyan
            6 => image::Rgb([20, 20, 20]),    // black
            _ => image::Rgb([240, 240, 240]), // white
        }
    });
    img.save(path).unwrap();
}

fn ensure_fixtures() {
    let dir = fixture_dir();
    std::fs::create_dir_all(&dir).unwrap();

    let dark = dir.join("dark-photo.png");
    if !dark.exists() {
        create_dark_photo(&dark);
    }
    let light = dir.join("light-photo.png");
    if !light.exists() {
        create_light_photo(&light);
    }
    let mono = dir.join("monochrome.png");
    if !mono.exists() {
        create_monochrome(&mono);
    }
    let colorful = dir.join("colorful.png");
    if !colorful.exists() {
        create_colorful(&colorful);
    }
}

/// Run the full pipeline on a fixture image and return serialized theme output.
fn run_pipeline(fixture_name: &str, mode: Option<ThemeMode>) -> String {
    ensure_fixtures();
    let path = fixture_dir().join(fixture_name);
    let pixels = load_and_prepare(&path).unwrap();
    let colors = extract_colors(&pixels, 16);
    let detected_mode = mode.unwrap_or_else(|| detect_mode(&pixels));
    let mut palette = assign_slots(&colors, detected_mode);
    enforce_contrast(&mut palette, DEFAULT_ACCENT_CONTRAST);
    GhosttyBackend.serialize(&palette, "test")
}

/// Validate the structural correctness of a theme output string.
fn validate_theme_structure(output: &str) {
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(
        lines.len(),
        22,
        "theme should have exactly 22 lines, got {}",
        lines.len()
    );

    // First 6 lines: special colors
    assert!(lines[0].starts_with("background = #"));
    assert!(lines[1].starts_with("foreground = #"));
    assert!(lines[2].starts_with("cursor-color = #"));
    assert!(lines[3].starts_with("cursor-text = #"));
    assert!(lines[4].starts_with("selection-background = #"));
    assert!(lines[5].starts_with("selection-foreground = #"));

    // 16 palette lines
    for i in 0..16 {
        let line = lines[6 + i];
        let prefix = format!("palette = {}=#", i);
        assert!(
            line.starts_with(&prefix),
            "line {} should start with '{prefix}', got '{line}'",
            6 + i
        );
    }

    // All hex values valid and lowercase
    for line in &lines {
        if let Some(pos) = line.find('#') {
            let hex = &line[pos..pos + 7];
            assert_eq!(hex.len(), 7);
            assert!(hex.starts_with('#'));
            assert!(
                hex[1..].chars().all(|c| c.is_ascii_hexdigit()),
                "invalid hex: '{hex}' in '{line}'"
            );
            assert_eq!(hex, &hex.to_lowercase(), "hex not lowercase: '{hex}'");
        }
    }
}

// ---------------------------------------------------------------------------
// Snapshot tests
// ---------------------------------------------------------------------------

/// Generate or verify a snapshot for a given fixture.
fn snapshot_test(fixture: &str) {
    let output = run_pipeline(fixture, None);
    validate_theme_structure(&output);

    let snap_dir = snapshot_dir();
    std::fs::create_dir_all(&snap_dir).unwrap();

    let snap_name = fixture.replace('.', "_") + ".snap";
    let snap_path = snap_dir.join(&snap_name);

    if std::env::var("UPDATE_SNAPSHOTS").is_ok() || !snap_path.exists() {
        std::fs::write(&snap_path, &output).unwrap();
        return;
    }

    let expected = std::fs::read_to_string(&snap_path).unwrap();
    assert_eq!(
        output, expected,
        "snapshot mismatch for {fixture}. Run with UPDATE_SNAPSHOTS=1 to update."
    );
}

#[test]
fn snapshot_dark_photo() {
    snapshot_test("dark-photo.png");
}

#[test]
fn snapshot_light_photo() {
    snapshot_test("light-photo.png");
}

#[test]
fn snapshot_monochrome() {
    snapshot_test("monochrome.png");
}

#[test]
fn snapshot_colorful() {
    snapshot_test("colorful.png");
}

// ---------------------------------------------------------------------------
// Pipeline validation tests
// ---------------------------------------------------------------------------

#[test]
fn dark_photo_detects_dark_mode() {
    ensure_fixtures();
    let pixels = load_and_prepare(&fixture_dir().join("dark-photo.png")).unwrap();
    assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
}

#[test]
fn light_photo_detects_light_mode() {
    ensure_fixtures();
    let pixels = load_and_prepare(&fixture_dir().join("light-photo.png")).unwrap();
    assert_eq!(detect_mode(&pixels), ThemeMode::Light);
}

#[test]
fn monochrome_produces_valid_theme() {
    let output = run_pipeline("monochrome.png", None);
    validate_theme_structure(&output);
}

#[test]
fn colorful_produces_valid_theme() {
    let output = run_pipeline("colorful.png", None);
    validate_theme_structure(&output);
}

#[test]
fn contrast_ratios_met_for_all_fixtures() {
    ensure_fixtures();
    for fixture in &[
        "dark-photo.png",
        "light-photo.png",
        "monochrome.png",
        "colorful.png",
    ] {
        let path = fixture_dir().join(fixture);
        let pixels = load_and_prepare(&path).unwrap();
        let colors = extract_colors(&pixels, 16);
        let mode = detect_mode(&pixels);
        let mut palette = assign_slots(&colors, mode);
        enforce_contrast(&mut palette, DEFAULT_ACCENT_CONTRAST);

        let bg = &palette.background;

        // Accent contrast >= 4.5:1
        for slot in (1..=6).chain(9..=14) {
            let ratio = Color::contrast_ratio(&palette.slots[slot], bg);
            assert!(
                ratio >= 4.5,
                "{fixture}: slot {slot} contrast {ratio:.2} < 4.5"
            );
        }

        // Foreground contrast >= 7:1
        let fg_ratio = Color::contrast_ratio(&palette.foreground, bg);
        assert!(
            fg_ratio >= 7.0,
            "{fixture}: foreground contrast {fg_ratio:.2} < 7.0"
        );

        // Bright black contrast >= 3:1
        let s8_ratio = Color::contrast_ratio(&palette.slots[8], bg);
        assert!(
            s8_ratio >= 3.0,
            "{fixture}: slot 8 contrast {s8_ratio:.2} < 3.0"
        );
    }
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    /// Generate a random synthetic image as a Vec<[u8; 3]> pixel buffer.
    fn arb_pixel_buffer() -> impl Strategy<Value = Vec<[u8; 3]>> {
        // 4x4 to 16x16 images with random pixel colors
        (4u32..=16u32, 4u32..=16u32).prop_flat_map(|(w, h)| {
            proptest::collection::vec(proptest::array::uniform3(0u8..=255u8), (w * h) as usize)
        })
    }

    fn pixels_to_lab(pixels: &[[u8; 3]]) -> Vec<palette::Lab> {
        use palette::{IntoColor, Srgb};
        pixels
            .iter()
            .map(|p| {
                let srgb: Srgb<f32> = Srgb::new(p[0], p[1], p[2]).into_format();
                srgb.into_color()
            })
            .collect()
    }

    proptest! {
        #[test]
        fn theme_always_has_22_lines(pixels in arb_pixel_buffer()) {
            let lab_pixels = pixels_to_lab(&pixels);
            let colors = extract_colors(&lab_pixels, 16);
            let mode = detect_mode(&lab_pixels);
            let mut palette = assign_slots(&colors, mode);
            enforce_contrast(&mut palette, DEFAULT_ACCENT_CONTRAST);
            let output = GhosttyBackend.serialize(&palette, "test");
            let line_count = output.lines().count();
            prop_assert_eq!(line_count, 22, "expected 22 lines, got {}", line_count);
        }

        #[test]
        fn all_hex_values_valid(pixels in arb_pixel_buffer()) {
            let lab_pixels = pixels_to_lab(&pixels);
            let colors = extract_colors(&lab_pixels, 16);
            let mode = detect_mode(&lab_pixels);
            let mut palette = assign_slots(&colors, mode);
            enforce_contrast(&mut palette, DEFAULT_ACCENT_CONTRAST);
            let output = GhosttyBackend.serialize(&palette, "test");

            let hex_re = regex::Regex::new(r"#[0-9a-f]{6}").unwrap();
            for line in output.lines() {
                if let Some(pos) = line.find('#') {
                    let hex = &line[pos..pos + 7];
                    prop_assert!(hex_re.is_match(hex), "invalid hex: '{}'", hex);
                }
            }
        }

        #[test]
        fn accent_contrast_always_met(pixels in arb_pixel_buffer()) {
            let lab_pixels = pixels_to_lab(&pixels);
            let colors = extract_colors(&lab_pixels, 16);
            let mode = detect_mode(&lab_pixels);
            let mut palette = assign_slots(&colors, mode);
            enforce_contrast(&mut palette, DEFAULT_ACCENT_CONTRAST);

            let bg = &palette.background;
            for slot in (1..=6).chain(9..=14) {
                let ratio = Color::contrast_ratio(&palette.slots[slot], bg);
                prop_assert!(
                    ratio >= 4.5,
                    "slot {} contrast {:.2} < 4.5",
                    slot,
                    ratio
                );
            }

            let fg_ratio = Color::contrast_ratio(&palette.foreground, bg);
            prop_assert!(fg_ratio >= 7.0, "foreground contrast {:.2} < 7.0", fg_ratio);
        }
    }
}

// ---------------------------------------------------------------------------
// CLI integration tests (run the actual binary)
// ---------------------------------------------------------------------------

fn cargo_bin() -> PathBuf {
    // Build the binary in test mode and return its path
    let output = Command::new("cargo")
        .args(["build", "--quiet"])
        .output()
        .expect("failed to build binary");
    assert!(output.status.success(), "cargo build failed");

    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join("debug")
        .join("nuri")
}

#[test]
fn cli_stdout_produces_valid_theme() {
    ensure_fixtures();
    let bin = cargo_bin();
    let output = Command::new(&bin)
        .arg(fixture_dir().join("dark-photo.png"))
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "binary exited with error");
    let stdout = String::from_utf8_lossy(&output.stdout);
    validate_theme_structure(&stdout);
}

#[test]
fn cli_mode_flag_works() {
    ensure_fixtures();
    let bin = cargo_bin();

    // Dark mode
    let output = Command::new(&bin)
        .args([
            fixture_dir().join("light-photo.png").to_str().unwrap(),
            "--mode",
            "dark",
        ])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    validate_theme_structure(&String::from_utf8_lossy(&output.stdout));

    // Light mode
    let output = Command::new(&bin)
        .args([
            fixture_dir().join("dark-photo.png").to_str().unwrap(),
            "--mode",
            "light",
        ])
        .output()
        .expect("failed to run binary");
    assert!(output.status.success());
    validate_theme_structure(&String::from_utf8_lossy(&output.stdout));
}

#[test]
fn cli_output_flag_writes_file() {
    ensure_fixtures();
    let bin = cargo_bin();
    let tmp = std::env::temp_dir().join("nuri-test-cli-output");
    std::fs::create_dir_all(&tmp).unwrap();
    let out_path = tmp.join("test-theme-out");

    let output = Command::new(&bin)
        .args([
            fixture_dir().join("dark-photo.png").to_str().unwrap(),
            "--output",
            out_path.to_str().unwrap(),
        ])
        .output()
        .expect("failed to run binary");

    assert!(output.status.success());
    assert!(out_path.exists(), "output file should be created");

    let content = std::fs::read_to_string(&out_path).unwrap();
    validate_theme_structure(&content);

    // Cleanup
    std::fs::remove_dir_all(&tmp).unwrap();
}

#[test]
fn cli_help_output() {
    let bin = cargo_bin();
    let output = Command::new(&bin)
        .arg("--help")
        .output()
        .expect("failed to run binary");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("nuri"));
    assert!(stdout.contains("--mode"));
    assert!(stdout.contains("--install"));
    assert!(stdout.contains("--no-clobber"));
    assert!(stdout.contains("--min-contrast"));
}

#[test]
fn cli_file_not_found_error() {
    let bin = cargo_bin();
    let output = Command::new(&bin)
        .arg("/nonexistent/image.png")
        .output()
        .expect("failed to run binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("file not found") || stderr.contains("No such file"),
        "expected file-not-found error, got: {stderr}"
    );
}

#[test]
fn cli_unsupported_format_error() {
    ensure_fixtures();
    let bin = cargo_bin();
    let output = Command::new(&bin)
        .arg(fixture_dir().join("not_an_image.txt"))
        .output()
        .expect("failed to run binary");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unsupported") || stderr.contains("Unsupported"),
        "expected unsupported format error, got: {stderr}"
    );
}
