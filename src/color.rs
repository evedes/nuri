use anyhow::{bail, Result};
use palette::{FromColor, IntoColor, Lab, Oklch, Srgb};

/// Core color type used throughout the pipeline.
/// Wraps sRGB u8 components and provides conversions to perceptual color spaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parse a hex color string like `#ff8800` or `#FF8800`.
    #[allow(dead_code)]
    pub fn from_hex(hex: &str) -> Result<Self> {
        let hex = hex.strip_prefix('#').unwrap_or(hex);
        if hex.len() != 6 {
            bail!(
                "invalid hex color: expected 6 hex digits, got {}",
                hex.len()
            );
        }
        let r = u8::from_str_radix(&hex[0..2], 16)?;
        let g = u8::from_str_radix(&hex[2..4], 16)?;
        let b = u8::from_str_radix(&hex[4..6], 16)?;
        Ok(Self { r, g, b })
    }

    /// Serialize to lowercase hex `#rrggbb`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }

    /// Convert to `palette::Srgb<u8>`.
    pub fn to_srgb_u8(self) -> Srgb<u8> {
        Srgb::new(self.r, self.g, self.b)
    }

    /// Create from `palette::Srgb<u8>`.
    #[allow(dead_code)]
    pub fn from_srgb_u8(srgb: Srgb<u8>) -> Self {
        Self {
            r: srgb.red,
            g: srgb.green,
            b: srgb.blue,
        }
    }

    /// Convert to CIELAB (for K-means clustering and deduplication).
    pub fn to_lab(self) -> Lab {
        let srgb_f32: Srgb<f32> = self.to_srgb_u8().into_format();
        srgb_f32.into_color()
    }

    /// Create from CIELAB.
    pub fn from_lab(lab: Lab) -> Self {
        let srgb_f32: Srgb<f32> = Srgb::from_color(lab);
        Self::from_srgb_f32_clamped(srgb_f32)
    }

    /// Convert to Oklch (for hue assignment, lightness/chroma adjustments).
    pub fn to_oklch(self) -> Oklch {
        let srgb_f32: Srgb<f32> = self.to_srgb_u8().into_format();
        srgb_f32.into_color()
    }

    /// Create from Oklch.
    pub fn from_oklch(oklch: Oklch) -> Self {
        let srgb_f32: Srgb<f32> = Srgb::from_color(oklch);
        Self::from_srgb_f32_clamped(srgb_f32)
    }

    /// Clamp an Srgb<f32> to [0, 1] and convert to Color.
    fn from_srgb_f32_clamped(srgb: Srgb<f32>) -> Self {
        let r = (srgb.red.clamp(0.0, 1.0) * 255.0).round() as u8;
        let g = (srgb.green.clamp(0.0, 1.0) * 255.0).round() as u8;
        let b = (srgb.blue.clamp(0.0, 1.0) * 255.0).round() as u8;
        Self { r, g, b }
    }

    /// WCAG 2.0 relative luminance.
    ///
    /// Linearizes each sRGB channel, then computes the weighted sum.
    pub fn relative_luminance(self) -> f32 {
        fn linearize(c: u8) -> f32 {
            let c = c as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        }
        let r = linearize(self.r);
        let g = linearize(self.g);
        let b = linearize(self.b);
        0.2126 * r + 0.7152 * g + 0.0722 * b
    }

    /// WCAG 2.0 contrast ratio between two colors.
    ///
    /// Returns a value in [1, 21]. Higher means more contrast.
    pub fn contrast_ratio(c1: &Color, c2: &Color) -> f32 {
        let l1 = c1.relative_luminance();
        let l2 = c2.relative_luminance();
        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// Adjust Oklch lightness by `delta`. Positive = lighter, negative = darker.
    /// Lightness is clamped to [0, 1].
    pub fn adjust_lightness(self, delta: f32) -> Color {
        let mut oklch = self.to_oklch();
        oklch.l = (oklch.l + delta).clamp(0.0, 1.0);
        Color::from_oklch(oklch)
    }

    /// Adjust Oklch chroma by `delta`. Positive = more saturated, negative = less.
    /// Chroma is clamped to [0, 0.4].
    #[allow(dead_code)]
    pub fn adjust_chroma(self, delta: f32) -> Color {
        let mut oklch = self.to_oklch();
        oklch.chroma = (oklch.chroma + delta).clamp(0.0, 0.4);
        Color::from_oklch(oklch)
    }
}

impl std::fmt::Display for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BLACK: Color = Color { r: 0, g: 0, b: 0 };
    const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
    };

    #[test]
    fn hex_round_trip() {
        let original = Color::from_hex("#ff8800").unwrap();
        assert_eq!(original.r, 255);
        assert_eq!(original.g, 136);
        assert_eq!(original.b, 0);
        assert_eq!(original.to_hex(), "#ff8800");
    }

    #[test]
    fn hex_uppercase_input() {
        let color = Color::from_hex("#FF8800").unwrap();
        assert_eq!(color.to_hex(), "#ff8800");
    }

    #[test]
    fn hex_without_hash() {
        let color = Color::from_hex("aabbcc").unwrap();
        assert_eq!(color.to_hex(), "#aabbcc");
    }

    #[test]
    fn hex_invalid_length() {
        assert!(Color::from_hex("#fff").is_err());
    }

    #[test]
    fn hex_invalid_chars() {
        assert!(Color::from_hex("#gggggg").is_err());
    }

    #[test]
    fn srgb_to_lab_round_trip() {
        let colors = [
            Color::new(200, 100, 50),
            Color::new(0, 255, 0),
            Color::new(128, 128, 128),
            BLACK,
            WHITE,
        ];
        for original in colors {
            let lab = original.to_lab();
            let recovered = Color::from_lab(lab);
            assert!(
                (original.r as i16 - recovered.r as i16).unsigned_abs() <= 1,
                "R mismatch for {:?}: {} vs {}",
                original,
                original.r,
                recovered.r
            );
            assert!(
                (original.g as i16 - recovered.g as i16).unsigned_abs() <= 1,
                "G mismatch for {:?}: {} vs {}",
                original,
                original.g,
                recovered.g
            );
            assert!(
                (original.b as i16 - recovered.b as i16).unsigned_abs() <= 1,
                "B mismatch for {:?}: {} vs {}",
                original,
                original.b,
                recovered.b
            );
        }
    }

    #[test]
    fn srgb_to_oklch_round_trip() {
        let colors = [
            Color::new(200, 100, 50),
            Color::new(0, 255, 0),
            Color::new(128, 128, 128),
            WHITE,
        ];
        for original in colors {
            let oklch = original.to_oklch();
            let recovered = Color::from_oklch(oklch);
            assert!(
                (original.r as i16 - recovered.r as i16).unsigned_abs() <= 1,
                "R mismatch for {:?}: {} vs {}",
                original,
                original.r,
                recovered.r
            );
            assert!(
                (original.g as i16 - recovered.g as i16).unsigned_abs() <= 1,
                "G mismatch for {:?}: {} vs {}",
                original,
                original.g,
                recovered.g
            );
            assert!(
                (original.b as i16 - recovered.b as i16).unsigned_abs() <= 1,
                "B mismatch for {:?}: {} vs {}",
                original,
                original.b,
                recovered.b
            );
        }
    }

    #[test]
    fn contrast_ratio_black_white() {
        let ratio = Color::contrast_ratio(&BLACK, &WHITE);
        assert!(
            (ratio - 21.0).abs() < 0.1,
            "black/white contrast should be ~21:1, got {ratio}"
        );
    }

    #[test]
    fn contrast_ratio_same_color() {
        let gray = Color::new(128, 128, 128);
        let ratio = Color::contrast_ratio(&gray, &gray);
        assert!(
            (ratio - 1.0).abs() < 0.001,
            "same color contrast should be 1:1, got {ratio}"
        );
    }

    #[test]
    fn contrast_ratio_is_symmetric() {
        let a = Color::new(200, 50, 50);
        let b = Color::new(50, 200, 50);
        let ratio_ab = Color::contrast_ratio(&a, &b);
        let ratio_ba = Color::contrast_ratio(&b, &a);
        assert!(
            (ratio_ab - ratio_ba).abs() < 0.001,
            "contrast ratio should be symmetric: {ratio_ab} vs {ratio_ba}"
        );
    }

    #[test]
    fn contrast_ratio_mid_gray_vs_black() {
        // sRGB(119,119,119) has relative luminance ~0.184
        // Contrast vs black: (0.184 + 0.05) / (0.0 + 0.05) ≈ 4.68
        let gray = Color::new(119, 119, 119);
        let ratio = Color::contrast_ratio(&gray, &BLACK);
        assert!(
            ratio > 4.5 && ratio < 5.0,
            "mid-gray vs black should be ~4.7:1, got {ratio}"
        );
    }

    #[test]
    fn relative_luminance_black() {
        assert!(BLACK.relative_luminance() < 0.001);
    }

    #[test]
    fn relative_luminance_white() {
        assert!((WHITE.relative_luminance() - 1.0).abs() < 0.001);
    }

    #[test]
    fn adjust_lightness_increases() {
        let dark = Color::new(50, 50, 50);
        let lighter = dark.adjust_lightness(0.2);
        assert!(
            lighter.relative_luminance() > dark.relative_luminance(),
            "increasing lightness should increase luminance"
        );
    }

    #[test]
    fn adjust_lightness_clamps() {
        let result = WHITE.adjust_lightness(1.0);
        // Should not panic, lightness clamped to 1.0
        assert!(result.relative_luminance() > 0.9);
    }

    #[test]
    fn adjust_chroma_preserves_approximate_hue() {
        let color = Color::new(200, 50, 50); // reddish
        let desaturated = color.adjust_chroma(-0.05);

        let original_oklch = color.to_oklch();
        let adjusted_oklch = desaturated.to_oklch();

        // Hue should stay approximately the same
        let hue_diff = (f32::from(original_oklch.hue) - f32::from(adjusted_oklch.hue)).abs();
        assert!(
            hue_diff < 5.0 || hue_diff > 355.0,
            "hue should be preserved, diff was {hue_diff}"
        );
    }

    #[test]
    fn display_matches_to_hex() {
        let color = Color::new(171, 205, 239);
        assert_eq!(format!("{color}"), color.to_hex());
    }
}
