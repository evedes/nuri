use crate::color::Color;
use crate::pipeline::assign::AnsiPalette;

/// Minimum contrast ratio for accent colors (slots 1-6, 9-14) vs background.
const ACCENT_MIN_CONTRAST: f32 = 4.5;

/// Minimum contrast ratio for foreground vs background.
const FOREGROUND_MIN_CONTRAST: f32 = 7.0;

/// Minimum contrast ratio for bright black (slot 8) vs background.
const BRIGHT_BLACK_MIN_CONTRAST: f32 = 3.0;

/// Oklch lightness adjustment step per iteration.
const L_STEP: f32 = 0.01;

/// Maximum adjustment iterations to prevent infinite loops.
const MAX_ITERATIONS: usize = 100;

/// Adjust palette colors to meet WCAG contrast minimums against the background.
///
/// Only Oklch lightness is adjusted — hue and chroma are preserved.
/// Direction is inferred from background luminance: lighten for dark themes,
/// darken for light themes.
pub fn enforce_contrast(palette: &mut AnsiPalette) {
    let bg = palette.background;
    let l_direction = if bg.relative_luminance() < 0.5 {
        L_STEP
    } else {
        -L_STEP
    };

    // Accent colors (slots 1-6, 9-14) vs background: ≥ 4.5:1
    for slot in (1..=6).chain(9..=14) {
        palette.slots[slot] =
            adjust_to_contrast(palette.slots[slot], bg, ACCENT_MIN_CONTRAST, l_direction);
    }

    // Foreground (slot 15) vs background: ≥ 7:1
    palette.slots[15] =
        adjust_to_contrast(palette.slots[15], bg, FOREGROUND_MIN_CONTRAST, l_direction);
    palette.foreground = palette.slots[15];
    palette.cursor_color = palette.foreground;
    palette.selection_fg = palette.foreground;

    // Bright black (slot 8) vs background: ≥ 3:1
    palette.slots[8] =
        adjust_to_contrast(palette.slots[8], bg, BRIGHT_BLACK_MIN_CONTRAST, l_direction);
}

/// Iteratively adjust a color's Oklch lightness until it meets the contrast target.
fn adjust_to_contrast(color: Color, background: Color, min_ratio: f32, l_step: f32) -> Color {
    let mut current = color;
    for _ in 0..MAX_ITERATIONS {
        if Color::contrast_ratio(&current, &background) >= min_ratio {
            return current;
        }
        current = current.adjust_lightness(l_step);
    }
    current
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ThemeMode;
    use crate::pipeline::assign::assign_slots;
    use crate::pipeline::extract::ExtractedColor;
    use palette::Oklch;

    fn make_extracted(l: f32, chroma: f32, hue: f32, weight: f32) -> ExtractedColor {
        ExtractedColor {
            color: Color::from_oklch(Oklch::new(l, chroma, hue)),
            weight,
        }
    }

    #[test]
    fn low_contrast_accent_gets_adjusted() {
        // Create a dark palette with a very dark red (low contrast against dark bg)
        let colors = vec![
            make_extracted(0.20, 0.15, 25.0, 0.15), // dark red → low contrast
            make_extracted(0.60, 0.20, 145.0, 0.10), // green
            make_extracted(0.70, 0.20, 90.0, 0.10), // yellow
            make_extracted(0.55, 0.20, 260.0, 0.10), // blue
            make_extracted(0.60, 0.20, 325.0, 0.10), // magenta
            make_extracted(0.65, 0.20, 195.0, 0.10), // cyan
            make_extracted(0.05, 0.01, 0.0, 0.20),  // very dark base
            make_extracted(0.95, 0.01, 0.0, 0.15),  // light base
        ];

        let mut palette = assign_slots(&colors, ThemeMode::Dark);
        let red_before = palette.slots[1];
        let ratio_before = Color::contrast_ratio(&red_before, &palette.background);

        enforce_contrast(&mut palette);
        let ratio_after = Color::contrast_ratio(&palette.slots[1], &palette.background);

        // Contrast should have improved
        assert!(
            ratio_after >= ratio_before,
            "contrast should improve: {ratio_before:.2} → {ratio_after:.2}"
        );
        assert!(
            ratio_after >= 4.5,
            "accent contrast should be ≥ 4.5:1 after enforcement, got {ratio_after:.2}"
        );
    }

    #[test]
    fn already_passing_palette_unchanged() {
        // Create a palette with good contrast already
        let colors = vec![
            make_extracted(0.70, 0.20, 25.0, 0.12),
            make_extracted(0.70, 0.20, 145.0, 0.12),
            make_extracted(0.80, 0.20, 90.0, 0.12),
            make_extracted(0.65, 0.20, 260.0, 0.12),
            make_extracted(0.70, 0.20, 325.0, 0.12),
            make_extracted(0.75, 0.20, 195.0, 0.10),
            make_extracted(0.05, 0.01, 0.0, 0.15),
            make_extracted(0.97, 0.01, 0.0, 0.15),
        ];

        let mut palette = assign_slots(&colors, ThemeMode::Dark);
        let slots_before = palette.slots;

        enforce_contrast(&mut palette);

        // Check which slots changed (some might need no adjustment)
        for slot in (1..=6).chain(9..=14) {
            let ratio = Color::contrast_ratio(&palette.slots[slot], &palette.background);
            if Color::contrast_ratio(&slots_before[slot], &palette.background) >= 4.5 {
                assert_eq!(
                    palette.slots[slot], slots_before[slot],
                    "slot {slot} had sufficient contrast and should be unchanged"
                );
            }
            assert!(
                ratio >= 4.5,
                "slot {slot} should have ≥ 4.5:1 contrast, got {ratio:.2}"
            );
        }
    }

    #[test]
    fn all_thresholds_met_after_enforcement() {
        // Test with a variety of input palettes
        let test_cases = vec![
            // Case 1: mostly dark colors on dark bg
            vec![
                make_extracted(0.25, 0.10, 25.0, 0.15),
                make_extracted(0.30, 0.10, 145.0, 0.15),
                make_extracted(0.20, 0.10, 90.0, 0.15),
                make_extracted(0.25, 0.10, 260.0, 0.15),
                make_extracted(0.30, 0.10, 325.0, 0.10),
                make_extracted(0.25, 0.10, 195.0, 0.10),
                make_extracted(0.05, 0.01, 0.0, 0.10),
                make_extracted(0.95, 0.01, 0.0, 0.10),
            ],
            // Case 2: light mode palette
            vec![
                make_extracted(0.80, 0.10, 25.0, 0.15),
                make_extracted(0.75, 0.10, 145.0, 0.15),
                make_extracted(0.85, 0.10, 90.0, 0.15),
                make_extracted(0.80, 0.10, 260.0, 0.15),
                make_extracted(0.75, 0.10, 325.0, 0.10),
                make_extracted(0.80, 0.10, 195.0, 0.10),
                make_extracted(0.10, 0.01, 0.0, 0.10),
                make_extracted(0.95, 0.01, 0.0, 0.10),
            ],
        ];

        for (case_idx, colors) in test_cases.into_iter().enumerate() {
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                let mut palette = assign_slots(&colors, mode);
                enforce_contrast(&mut palette);

                let bg = palette.background;

                // Check accent contrast
                for slot in (1..=6).chain(9..=14) {
                    let ratio = Color::contrast_ratio(&palette.slots[slot], &bg);
                    assert!(
                        ratio >= ACCENT_MIN_CONTRAST,
                        "case {case_idx} {mode:?}: slot {slot} contrast {ratio:.2} < {ACCENT_MIN_CONTRAST}"
                    );
                }

                // Check foreground contrast
                let fg_ratio = Color::contrast_ratio(&palette.foreground, &bg);
                assert!(
                    fg_ratio >= FOREGROUND_MIN_CONTRAST,
                    "case {case_idx} {mode:?}: foreground contrast {fg_ratio:.2} < {FOREGROUND_MIN_CONTRAST}"
                );

                // Check bright black contrast
                let s8_ratio = Color::contrast_ratio(&palette.slots[8], &bg);
                assert!(
                    s8_ratio >= BRIGHT_BLACK_MIN_CONTRAST,
                    "case {case_idx} {mode:?}: slot 8 contrast {s8_ratio:.2} < {BRIGHT_BLACK_MIN_CONTRAST}"
                );
            }
        }
    }

    #[test]
    fn hue_preserved_after_adjustment() {
        let colors = vec![
            make_extracted(0.20, 0.15, 25.0, 0.20),
            make_extracted(0.60, 0.20, 145.0, 0.10),
            make_extracted(0.70, 0.20, 90.0, 0.10),
            make_extracted(0.55, 0.20, 260.0, 0.10),
            make_extracted(0.60, 0.20, 325.0, 0.10),
            make_extracted(0.65, 0.20, 195.0, 0.10),
            make_extracted(0.05, 0.01, 0.0, 0.20),
            make_extracted(0.95, 0.01, 0.0, 0.10),
        ];

        let mut palette = assign_slots(&colors, ThemeMode::Dark);
        let hues_before: Vec<f32> = (1..=6)
            .map(|i| f32::from(palette.slots[i].to_oklch().hue))
            .collect();

        enforce_contrast(&mut palette);

        for (i, &hue_before) in (1..=6).zip(hues_before.iter()) {
            let hue_after = f32::from(palette.slots[i].to_oklch().hue);
            let diff = (hue_before - hue_after).abs();
            let diff = if diff > 180.0 { 360.0 - diff } else { diff };
            assert!(
                diff < 15.0,
                "slot {i} hue should be preserved: {hue_before:.1}° → {hue_after:.1}° (diff {diff:.1}°)"
            );
        }
    }

    #[test]
    fn foreground_synced_with_slot_15() {
        let colors = vec![
            make_extracted(0.50, 0.15, 25.0, 0.30),
            make_extracted(0.10, 0.01, 0.0, 0.30),
            make_extracted(0.95, 0.01, 0.0, 0.40),
        ];

        let mut palette = assign_slots(&colors, ThemeMode::Dark);
        enforce_contrast(&mut palette);

        assert_eq!(
            palette.foreground, palette.slots[15],
            "foreground should be synced with slot 15 after enforcement"
        );
        assert_eq!(
            palette.cursor_color, palette.foreground,
            "cursor_color should be synced with foreground"
        );
    }
}
