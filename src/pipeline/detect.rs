use palette::Lab;

use crate::cli::ThemeMode;

/// Lightness threshold: pixels with mean L above this are considered light.
const LIGHT_THRESHOLD: f32 = 55.0;

/// Detect whether the image is predominantly dark or light.
///
/// Computes the mean CIE-Lab L channel across all pixels. If the mean
/// exceeds [`LIGHT_THRESHOLD`] the image is classified as light, otherwise dark.
pub fn detect_mode(pixels: &[Lab]) -> ThemeMode {
    if pixels.is_empty() {
        return ThemeMode::Dark;
    }

    let mean_l: f32 = pixels.iter().map(|p| p.l).sum::<f32>() / pixels.len() as f32;

    if mean_l > LIGHT_THRESHOLD {
        ThemeMode::Light
    } else {
        ThemeMode::Dark
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_black_is_dark() {
        let pixels = vec![Lab::new(0.0, 0.0, 0.0); 100];
        assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
    }

    #[test]
    fn all_white_is_light() {
        let pixels = vec![Lab::new(100.0, 0.0, 0.0); 100];
        assert_eq!(detect_mode(&pixels), ThemeMode::Light);
    }

    #[test]
    fn just_below_threshold_is_dark() {
        let pixels = vec![Lab::new(54.9, 0.0, 0.0); 100];
        assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
    }

    #[test]
    fn just_above_threshold_is_light() {
        let pixels = vec![Lab::new(55.1, 0.0, 0.0); 100];
        assert_eq!(detect_mode(&pixels), ThemeMode::Light);
    }

    #[test]
    fn exactly_at_threshold_is_dark() {
        let pixels = vec![Lab::new(55.0, 0.0, 0.0); 100];
        assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
    }

    #[test]
    fn empty_pixels_defaults_to_dark() {
        let pixels: Vec<Lab> = vec![];
        assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
    }

    #[test]
    fn mixed_pixels_below_threshold() {
        // Average L = (20 + 80) / 2 = 50 → Dark
        let pixels = vec![Lab::new(20.0, 0.0, 0.0), Lab::new(80.0, 0.0, 0.0)];
        assert_eq!(detect_mode(&pixels), ThemeMode::Dark);
    }

    #[test]
    fn mixed_pixels_above_threshold() {
        // Average L = (40 + 80) / 2 = 60 → Light
        let pixels = vec![Lab::new(40.0, 0.0, 0.0), Lab::new(80.0, 0.0, 0.0)];
        assert_eq!(detect_mode(&pixels), ThemeMode::Light);
    }
}
