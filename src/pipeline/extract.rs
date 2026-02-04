use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use kmeans_colors::get_kmeans_hamerly;
use palette::{IntoColor, Lab, Srgb};

use crate::color::Color;

/// A color extracted from the image with its cluster weight.
#[derive(Debug, Clone)]
pub struct ExtractedColor {
    pub color: Color,
    pub weight: f32,
}

const MAX_DIM: u32 = 256;
const MAX_ITER: usize = 20;
const CONVERGE: f32 = 5.0;
const DEDUP_THRESHOLD: f32 = 25.0; // ΔE² < 25 means ΔE < 5

/// Load an image, resize to fit within 256x256 (preserving aspect ratio),
/// and convert all pixels to CIELAB space.
pub fn load_and_prepare(path: &Path) -> Result<Vec<Lab>> {
    let img = image::open(path).with_context(|| {
        if !path.exists() {
            format!("file not found: {}", path.display())
        } else {
            format!(
                "unsupported or corrupt image: {}. Supported formats: PNG, JPEG, WebP, BMP, TIFF, GIF",
                path.display()
            )
        }
    })?;

    let img = if img.width() > MAX_DIM || img.height() > MAX_DIM {
        img.resize(MAX_DIM, MAX_DIM, FilterType::Lanczos3)
    } else {
        img
    };
    let rgb_img = img.to_rgb8();

    let pixels: Vec<Lab> = rgb_img
        .pixels()
        .map(|p| {
            let srgb: Srgb<f32> = Srgb::new(p[0], p[1], p[2]).into_format();
            srgb.into_color()
        })
        .collect();

    Ok(pixels)
}

/// Run K-means on LAB pixels to extract dominant colors.
///
/// Returns deduplicated colors sorted by weight (descending).
/// Uses Hamerly's algorithm with K-means++ initialization.
pub fn extract_colors(pixels: &[Lab], k: usize) -> Vec<ExtractedColor> {
    let seed = 42;
    let result = get_kmeans_hamerly(k, MAX_ITER, CONVERGE, false, pixels, seed);

    let total = pixels.len() as f32;

    // Count pixels per centroid to compute weights
    let mut counts = vec![0u32; k];
    for &idx in &result.indices {
        counts[idx as usize] += 1;
    }

    let mut colors: Vec<ExtractedColor> = result
        .centroids
        .iter()
        .enumerate()
        .filter(|(i, _)| counts[*i] > 0)
        .map(|(i, lab)| ExtractedColor {
            color: Color::from_lab(*lab),
            weight: counts[i] as f32 / total,
        })
        .collect();

    // Deduplicate centroids with ΔE < 5 (squared distance < 25)
    deduplicate(&mut colors);

    // Sort by weight descending
    colors.sort_by(|a, b| b.weight.partial_cmp(&a.weight).unwrap());

    colors
}

/// Merge colors that are too similar (ΔE < 5 in LAB space).
/// Keeps the first color and accumulates the weight.
fn deduplicate(colors: &mut Vec<ExtractedColor>) {
    let mut i = 0;
    while i < colors.len() {
        let mut j = i + 1;
        while j < colors.len() {
            let lab_i = colors[i].color.to_lab();
            let lab_j = colors[j].color.to_lab();
            let delta_e_sq = (lab_i.l - lab_j.l).powi(2)
                + (lab_i.a - lab_j.a).powi(2)
                + (lab_i.b - lab_j.b).powi(2);
            if delta_e_sq < DEDUP_THRESHOLD {
                colors[i].weight += colors[j].weight;
                colors.remove(j);
            } else {
                j += 1;
            }
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name)
    }

    // --- load_and_prepare tests ---

    #[test]
    fn load_4x4_png() {
        let path = fixture_path("4x4_test.png");
        create_test_image_solid(&path, 4, 4, [128, 128, 128]);

        let pixels = load_and_prepare(&path).unwrap();
        assert_eq!(pixels.len(), 16);
    }

    #[test]
    fn load_large_image_resizes() {
        let path = fixture_path("512x512_test.png");
        create_test_image_solid(&path, 512, 512, [128, 128, 128]);

        let pixels = load_and_prepare(&path).unwrap();
        assert_eq!(pixels.len(), 256 * 256);
    }

    #[test]
    fn load_nonsquare_preserves_aspect_ratio() {
        let path = fixture_path("512x256_test.png");
        create_test_image_solid(&path, 512, 256, [128, 128, 128]);

        let pixels = load_and_prepare(&path).unwrap();
        assert_eq!(pixels.len(), 256 * 128);
    }

    #[test]
    fn load_file_not_found() {
        let result = load_and_prepare(Path::new("/nonexistent/image.png"));
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("file not found") || err.contains("No such file"),
            "expected file-not-found error, got: {err}"
        );
    }

    #[test]
    fn load_unsupported_format() {
        let path = fixture_path("not_an_image.txt");
        std::fs::write(&path, "this is not an image").unwrap();

        let result = load_and_prepare(&path);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unsupported") || err.contains("Unsupported"),
            "expected unsupported format error, got: {err}"
        );
    }

    #[test]
    fn pixels_are_valid_lab() {
        let path = fixture_path("4x4_lab_test.png");
        create_test_image_gradient(&path, 4, 4);

        let pixels = load_and_prepare(&path).unwrap();
        for lab in &pixels {
            assert!(lab.l >= 0.0 && lab.l <= 100.0, "L out of range: {}", lab.l);
        }
    }

    // --- extract_colors tests ---

    #[test]
    fn uniform_image_produces_one_dominant_color() {
        // All pixels are the same red color
        let red_lab: Lab = Srgb::new(200u8, 50u8, 50u8)
            .into_format::<f32>()
            .into_color();
        let pixels = vec![red_lab; 1000];

        let colors = extract_colors(&pixels, 8);

        // After deduplication, all centroids should collapse into ~1 color
        assert!(
            colors.len() <= 2,
            "uniform image should produce ~1 color after dedup, got {}",
            colors.len()
        );
        // The dominant color should have nearly all the weight
        assert!(
            colors[0].weight > 0.8,
            "dominant color weight should be >0.8, got {}",
            colors[0].weight
        );
    }

    #[test]
    fn two_color_image_produces_two_dominant_colors() {
        // Half red, half blue
        let red_lab: Lab = Srgb::new(200u8, 50u8, 50u8)
            .into_format::<f32>()
            .into_color();
        let blue_lab: Lab = Srgb::new(50u8, 50u8, 200u8)
            .into_format::<f32>()
            .into_color();

        let mut pixels = vec![red_lab; 500];
        pixels.extend(vec![blue_lab; 500]);

        let colors = extract_colors(&pixels, 8);

        assert!(
            colors.len() >= 2,
            "two-color image should produce at least 2 colors, got {}",
            colors.len()
        );

        // Both dominant colors should have roughly equal weight
        let top_two_weight: f32 = colors.iter().take(2).map(|c| c.weight).sum();
        assert!(
            top_two_weight > 0.9,
            "top 2 colors should cover >90% of weight, got {}",
            top_two_weight
        );

        // Weights should be roughly balanced
        assert!(
            (colors[0].weight - colors[1].weight).abs() < 0.2,
            "weights should be roughly equal: {} vs {}",
            colors[0].weight,
            colors[1].weight
        );
    }

    #[test]
    fn results_sorted_by_weight_descending() {
        let red_lab: Lab = Srgb::new(200u8, 50u8, 50u8)
            .into_format::<f32>()
            .into_color();
        let blue_lab: Lab = Srgb::new(50u8, 50u8, 200u8)
            .into_format::<f32>()
            .into_color();
        let green_lab: Lab = Srgb::new(50u8, 200u8, 50u8)
            .into_format::<f32>()
            .into_color();

        let mut pixels = vec![red_lab; 600];
        pixels.extend(vec![blue_lab; 300]);
        pixels.extend(vec![green_lab; 100]);

        let colors = extract_colors(&pixels, 8);

        for window in colors.windows(2) {
            assert!(
                window[0].weight >= window[1].weight,
                "colors not sorted by weight: {} < {}",
                window[0].weight,
                window[1].weight
            );
        }
    }

    #[test]
    fn deduplication_merges_similar_colors() {
        // Create pixels with very slightly different shades of the same color
        let lab1: Lab = Lab::new(50.0, 20.0, 30.0);
        let lab2: Lab = Lab::new(51.0, 20.5, 30.5); // ΔE ≈ 1.2, should be merged

        let mut pixels = vec![lab1; 500];
        pixels.extend(vec![lab2; 500]);

        let colors = extract_colors(&pixels, 4);

        assert!(
            colors.len() <= 2,
            "near-identical colors should be deduplicated, got {}",
            colors.len()
        );
    }

    // --- test helpers ---

    fn create_test_image_solid(path: &Path, width: u32, height: u32, rgb: [u8; 3]) {
        let img = image::RgbImage::from_fn(width, height, |_, _| image::Rgb(rgb));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        img.save(path).unwrap();
    }

    fn create_test_image_gradient(path: &Path, width: u32, height: u32) {
        let img = image::RgbImage::from_fn(width, height, |x, y| {
            let r = ((x * 255) / width.max(1)) as u8;
            let g = ((y * 255) / height.max(1)) as u8;
            image::Rgb([r, g, 128])
        });
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        img.save(path).unwrap();
    }
}
