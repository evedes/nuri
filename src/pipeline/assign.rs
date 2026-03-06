use palette::Oklch;

use crate::cli::{AccentColor, AccentVariant, ThemeMode};
use crate::color::Color;
use crate::pipeline::extract::ExtractedColor;

/// The full ANSI palette plus special Ghostty theme colors.
#[derive(Debug, Clone)]
pub struct AnsiPalette {
    /// ANSI colors 0-15.
    pub slots: [Color; 16],
    pub background: Color,
    pub foreground: Color,
    pub cursor_color: Color,
    pub cursor_text: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

/// Target Oklch hue angles (degrees) for the six ANSI accent slots.
const TARGET_HUES: [(usize, f32); 6] = [
    (1, 25.0),  // Red
    (2, 145.0), // Green
    (3, 90.0),  // Yellow
    (4, 260.0), // Blue
    (5, 325.0), // Magenta
    (6, 195.0), // Cyan
];

/// Maximum hue distance (degrees) before we synthesize instead of using the candidate.
const MAX_HUE_DISTANCE: f32 = 60.0;

/// Oklch lightness increase for bright variants (slots 9-14).
const BRIGHT_L_DELTA: f32 = 0.12;

/// Minimum Oklch chroma to consider a candidate chromatic (not gray).
const MIN_CHROMA: f32 = 0.02;

/// Maximum chroma for background/dim base slots (preserves slight tint).
const BASE_MAX_CHROMA: f32 = 0.04;

/// Maximum chroma for text-emphasis slots to keep them near-neutral.
const TEXT_MAX_CHROMA: f32 = 0.02;

/// Angular distance between two hue values, wrapped to [0, 180].
fn hue_distance(a: f32, b: f32) -> f32 {
    let diff = (a - b).abs() % 360.0;
    if diff > 180.0 {
        360.0 - diff
    } else {
        diff
    }
}

/// Map extracted colors to the 16 ANSI palette slots plus special colors.
pub fn assign_slots(colors: &[ExtractedColor], mode: ThemeMode) -> AnsiPalette {
    assign_slots_with_accent(colors, mode, None, AccentVariant::Vibrant)
}

/// Map extracted colors to ANSI palette slots, optionally using a monochromatic accent.
pub fn assign_slots_with_accent(
    colors: &[ExtractedColor],
    mode: ThemeMode,
    accent: Option<AccentColor>,
    variant: AccentVariant,
) -> AnsiPalette {
    let mut slots = [Color::new(0, 0, 0); 16];

    let oklch_colors: Vec<Oklch> = colors.iter().map(|ec| ec.color.to_oklch()).collect();

    if let Some(accent_color) = accent {
        assign_monochromatic_accents(&oklch_colors, accent_color, variant, mode, &mut slots);
    } else {
        assign_accents(&oklch_colors, &mut slots);
    }
    assign_base_colors(&oklch_colors, mode, accent, &mut slots);
    assign_bright_variants(&mut slots);
    derive_special_colors(slots, mode)
}

/// Assign accent colors (slots 1-6) by hue proximity to target hues.
///
/// If no candidate is within [`MAX_HUE_DISTANCE`] of a target, the nearest
/// candidate's hue is rotated to the target in Oklch space (synthesis).
fn assign_accents(candidates: &[Oklch], slots: &mut [Color; 16]) {
    let chromatic: Vec<Oklch> = candidates
        .iter()
        .copied()
        .filter(|c| c.chroma > MIN_CHROMA)
        .collect();

    for &(slot, target_hue) in &TARGET_HUES {
        if let Some(best) = find_closest_by_hue(&chromatic, target_hue) {
            let dist = hue_distance(f32::from(best.hue), target_hue);
            if dist <= MAX_HUE_DISTANCE {
                slots[slot] = Color::from_oklch(best);
            } else {
                // Synthesize: rotate the nearest candidate's hue to the target
                let synth = Oklch::new(best.l, best.chroma, target_hue);
                slots[slot] = Color::from_oklch(synth);
            }
        } else {
            // No chromatic candidates — fully synthetic fallback
            slots[slot] = Color::from_oklch(Oklch::new(0.65, 0.15, target_hue));
        }
    }
}

/// Lightness offsets for each ANSI accent slot in monochromatic mode.
/// Spreads slots across different lightness levels to keep them distinguishable.
const MONO_SLOT_PARAMS: [(usize, f32, f32); 6] = [
    (1, 0.55, 0.18), // Red    → darkest accent
    (2, 0.65, 0.16), // Green  → mid
    (3, 0.75, 0.14), // Yellow → lighter
    (4, 0.50, 0.20), // Blue   → deep
    (5, 0.60, 0.17), // Magenta→ mid-low
    (6, 0.70, 0.15), // Cyan   → mid-high
];

/// Resolve the hue angle for an accent color (arbitrary for Gray, since chroma is 0).
fn accent_hue(accent: AccentColor) -> f32 {
    match accent {
        AccentColor::Blue => 260.0,
        AccentColor::Green => 145.0,
        AccentColor::Yellow => 90.0,
        AccentColor::Red => 25.0,
        AccentColor::Purple => 325.0,
        AccentColor::Gray => 0.0,
    }
}

/// Whether this accent is achromatic (zero chroma).
fn accent_is_gray(accent: AccentColor) -> bool {
    matches!(accent, AccentColor::Gray)
}

/// Assign all accent slots as shades of a single hue with a style variant.
fn assign_monochromatic_accents(
    candidates: &[Oklch],
    accent: AccentColor,
    variant: AccentVariant,
    mode: ThemeMode,
    slots: &mut [Color; 16],
) {
    let hue = accent_hue(accent);
    let (l_offset, chroma_scale) = variant.modifiers();

    let gray = accent_is_gray(accent);

    // Try to derive base chroma from the image's most chromatic candidate near this hue
    let base_chroma = if gray {
        0.0
    } else {
        let chromatic: Vec<Oklch> = candidates
            .iter()
            .copied()
            .filter(|c| c.chroma > MIN_CHROMA)
            .collect();
        find_closest_by_hue(&chromatic, hue)
            .map(|c| c.chroma.max(0.10))
            .unwrap_or(0.15)
    };

    for &(slot, l, slot_chroma) in &MONO_SLOT_PARAMS {
        let base_l = l + l_offset;
        let adjusted_l = if mode == ThemeMode::Light {
            (base_l - 0.15).max(0.25)
        } else {
            base_l.clamp(0.30, 0.90)
        };
        let chroma = if gray {
            0.0
        } else {
            (base_chroma * (slot_chroma / 0.15) * chroma_scale).clamp(0.03, 0.30)
        };
        slots[slot] = Color::from_oklch(Oklch::new(adjusted_l, chroma, hue));
    }
}

/// Generate 6 preview accent colors for a given accent + variant combo.
/// Used by the TUI overlay to show swatches without running the full pipeline.
pub fn preview_monochromatic(
    accent: AccentColor,
    variant: AccentVariant,
    mode: ThemeMode,
) -> [Color; 6] {
    let hue = accent_hue(accent);
    let (l_offset, chroma_scale) = variant.modifiers();
    let gray = accent_is_gray(accent);
    let base_chroma = 0.15;
    let mut out = [Color::new(0, 0, 0); 6];

    for (i, &(_slot, l, slot_chroma)) in MONO_SLOT_PARAMS.iter().enumerate() {
        let base_l = l + l_offset;
        let adjusted_l = if mode == ThemeMode::Light {
            (base_l - 0.15).max(0.25)
        } else {
            base_l.clamp(0.30, 0.90)
        };
        let chroma = if gray {
            0.0
        } else {
            (base_chroma * (slot_chroma / 0.15) * chroma_scale).clamp(0.03, 0.30)
        };
        out[i] = Color::from_oklch(Oklch::new(adjusted_l, chroma, hue));
    }
    out
}

/// Find the candidate with the smallest hue distance to `target_hue`.
fn find_closest_by_hue(candidates: &[Oklch], target_hue: f32) -> Option<Oklch> {
    candidates.iter().copied().min_by(|a, b| {
        hue_distance(f32::from(a.hue), target_hue)
            .partial_cmp(&hue_distance(f32::from(b.hue), target_hue))
            .unwrap()
    })
}

/// Chroma for accent-tinted background/dim slots.
const ACCENT_BASE_CHROMA: f32 = 0.01;
/// Chroma for accent-tinted text slots (white, bright white).
const ACCENT_TEXT_CHROMA: f32 = 0.015;

/// Assign base colors (slots 0, 7, 8, 15) based on theme mode.
///
/// Dark mode: slot 0 = darkest (L ≤ 0.15), slot 15 = lightest (L ~ 0.93).
/// Light mode: inverted — slot 0 = lightest, slot 15 = darkest.
///
/// When a monochromatic accent is active, base slots are tinted with the accent hue.
fn assign_base_colors(
    candidates: &[Oklch],
    mode: ThemeMode,
    accent: Option<AccentColor>,
    slots: &mut [Color; 16],
) {
    let darkest = candidates
        .iter()
        .copied()
        .min_by(|a, b| a.l.partial_cmp(&b.l).unwrap());
    let lightest = candidates
        .iter()
        .copied()
        .max_by(|a, b| a.l.partial_cmp(&b.l).unwrap());

    let dark_base = darkest.unwrap_or(Oklch::new(0.15, 0.0, 0.0));
    let light_base = lightest.unwrap_or(Oklch::new(0.93, 0.0, 0.0));

    // When accent is active, tint base slots with the accent hue (except gray)
    let (bg_hue, bg_chroma, text_hue, text_chroma) = if let Some(ac) = accent {
        if accent_is_gray(ac) {
            (0.0, 0.0, 0.0, 0.0)
        } else {
            let h = accent_hue(ac);
            (h, ACCENT_BASE_CHROMA, h, ACCENT_TEXT_CHROMA)
        }
    } else {
        (
            f32::from(dark_base.hue),
            dark_base.chroma.min(BASE_MAX_CHROMA),
            f32::from(light_base.hue),
            light_base.chroma.min(TEXT_MAX_CHROMA),
        )
    };

    match mode {
        ThemeMode::Dark => {
            // Slot 0 (black): darkest candidate, clamped to L ≤ 0.15
            slots[0] = Color::from_oklch(Oklch::new(dark_base.l.min(0.15), bg_chroma, bg_hue));
            // Slot 7 (white): light text, L ~ 0.85
            slots[7] = Color::from_oklch(Oklch::new(0.85, text_chroma, text_hue));
            // Slot 8 (bright black): dim text / comments, L ~ 0.40
            slots[8] = Color::from_oklch(Oklch::new(0.40, bg_chroma, bg_hue));
            // Slot 15 (bright white): brightest text, L ~ 0.93
            slots[15] = Color::from_oklch(Oklch::new(0.93, text_chroma, text_hue));
        }
        ThemeMode::Light => {
            // Inverted: slot 0 = lightest (background), slot 15 = darkest (foreground)
            slots[0] = Color::from_oklch(Oklch::new(light_base.l.max(0.93), text_chroma, text_hue));
            slots[7] = Color::from_oklch(Oklch::new(0.20, text_chroma, text_hue));
            slots[8] = Color::from_oklch(Oklch::new(0.60, bg_chroma, bg_hue));
            slots[15] = Color::from_oklch(Oklch::new(dark_base.l.min(0.15), text_chroma, text_hue));
        }
    }
}

/// Generate bright variants (slots 9-14) from normal accents (slots 1-6).
fn assign_bright_variants(slots: &mut [Color; 16]) {
    for i in 1..=6 {
        slots[i + 8] = slots[i].adjust_lightness(BRIGHT_L_DELTA);
    }
}

/// Derive special theme colors (background, foreground, cursor, selection).
///
/// Background = slot 0, foreground = slot 15 in both modes. The base color
/// inversion ensures slot 0 is dark in dark mode and light in light mode.
fn derive_special_colors(slots: [Color; 16], mode: ThemeMode) -> AnsiPalette {
    let background = slots[0];
    let foreground = slots[15];
    let cursor_color = foreground;
    let cursor_text = background;

    // Selection: blue accent (slot 4) with reduced chroma
    let sel = slots[4].to_oklch();
    let sel_l = match mode {
        ThemeMode::Dark => (sel.l + 0.1).min(1.0),
        ThemeMode::Light => (sel.l - 0.1).max(0.0),
    };
    let selection_bg = Color::from_oklch(Oklch::new(sel_l, (sel.chroma * 0.6).max(0.01), sel.hue));
    let selection_fg = foreground;

    AnsiPalette {
        slots,
        background,
        foreground,
        cursor_color,
        cursor_text,
        selection_bg,
        selection_fg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_extracted(l: f32, chroma: f32, hue: f32, weight: f32) -> ExtractedColor {
        ExtractedColor {
            color: Color::from_oklch(Oklch::new(l, chroma, hue)),
            weight,
        }
    }

    fn diverse_candidates() -> Vec<ExtractedColor> {
        vec![
            make_extracted(0.60, 0.20, 25.0, 0.12),  // Red
            make_extracted(0.60, 0.20, 145.0, 0.12), // Green
            make_extracted(0.70, 0.20, 90.0, 0.12),  // Yellow
            make_extracted(0.55, 0.20, 260.0, 0.12), // Blue
            make_extracted(0.60, 0.20, 325.0, 0.12), // Magenta
            make_extracted(0.65, 0.20, 195.0, 0.10), // Cyan
            make_extracted(0.10, 0.01, 0.0, 0.15),   // dark base
            make_extracted(0.95, 0.01, 0.0, 0.15),   // light base
        ]
    }

    #[test]
    fn diverse_hues_land_in_correct_slots() {
        let palette = assign_slots(&diverse_candidates(), ThemeMode::Dark);

        for &(slot, target_hue) in &TARGET_HUES {
            let oklch = palette.slots[slot].to_oklch();
            let dist = hue_distance(f32::from(oklch.hue), target_hue);
            // Tolerance accounts for hue drift from sRGB gamut clamping
            assert!(
                dist < 15.0,
                "slot {slot} hue {:.1}° should be near target {target_hue}°, distance {dist:.1}°",
                f32::from(oklch.hue)
            );
        }
    }

    #[test]
    fn gaps_filled_via_synthesis() {
        // Only Red and Blue — others must be synthesized
        let colors = vec![
            make_extracted(0.60, 0.20, 25.0, 0.40),
            make_extracted(0.55, 0.20, 260.0, 0.40),
            make_extracted(0.10, 0.01, 0.0, 0.15),
            make_extracted(0.95, 0.01, 0.0, 0.05),
        ];

        let palette = assign_slots(&colors, ThemeMode::Dark);

        // All accent slots should be non-black
        for &(slot, _) in &TARGET_HUES {
            let c = palette.slots[slot];
            assert!(
                c.r > 0 || c.g > 0 || c.b > 0,
                "slot {slot} should not be black"
            );
        }

        // Hue tolerance accounts for sRGB gamut clamping drift
        let green_hue = f32::from(palette.slots[2].to_oklch().hue);
        let green_dist = hue_distance(green_hue, 145.0);
        assert!(
            green_dist < 20.0,
            "synthesized green hue should be near 145°, got {green_hue:.1}° (dist {green_dist:.1}°)"
        );

        let yellow_hue = f32::from(palette.slots[3].to_oklch().hue);
        let yellow_dist = hue_distance(yellow_hue, 90.0);
        assert!(
            yellow_dist < 20.0,
            "synthesized yellow hue should be near 90°, got {yellow_hue:.1}° (dist {yellow_dist:.1}°)"
        );
    }

    #[test]
    fn bright_variants_are_lighter() {
        let palette = assign_slots(&diverse_candidates(), ThemeMode::Dark);

        for i in 1..=6 {
            let normal_l = palette.slots[i].to_oklch().l;
            let bright_l = palette.slots[i + 8].to_oklch().l;
            assert!(
                bright_l > normal_l,
                "bright slot {} (L={bright_l:.3}) should be lighter than slot {i} (L={normal_l:.3})",
                i + 8
            );
        }
    }

    #[test]
    fn dark_mode_base_colors_correct_lightness() {
        let palette = assign_slots(&diverse_candidates(), ThemeMode::Dark);

        let s0 = palette.slots[0].to_oklch().l;
        let s7 = palette.slots[7].to_oklch().l;
        let s8 = palette.slots[8].to_oklch().l;
        let s15 = palette.slots[15].to_oklch().l;

        assert!(s0 <= 0.16, "slot 0 L should be ≤ 0.15, got {s0:.3}");
        assert!(
            (s7 - 0.85).abs() < 0.05,
            "slot 7 L should be ~0.85, got {s7:.3}"
        );
        assert!(
            (s8 - 0.40).abs() < 0.05,
            "slot 8 L should be ~0.40, got {s8:.3}"
        );
        assert!(
            (s15 - 0.93).abs() < 0.05,
            "slot 15 L should be ~0.93, got {s15:.3}"
        );
    }

    #[test]
    fn light_mode_base_colors_inverted() {
        let palette = assign_slots(&diverse_candidates(), ThemeMode::Light);

        let s0 = palette.slots[0].to_oklch().l;
        let s15 = palette.slots[15].to_oklch().l;

        assert!(
            s0 > 0.90,
            "light mode slot 0 should be very light, got L={s0:.3}"
        );
        assert!(
            s15 < 0.20,
            "light mode slot 15 should be very dark, got L={s15:.3}"
        );
    }

    #[test]
    fn special_colors_derived_correctly() {
        let palette = assign_slots(&diverse_candidates(), ThemeMode::Dark);

        assert_eq!(palette.background, palette.slots[0]);
        assert_eq!(palette.foreground, palette.slots[15]);
        assert_eq!(palette.cursor_color, palette.foreground);
        assert_eq!(palette.cursor_text, palette.background);
        assert_eq!(palette.selection_fg, palette.foreground);
    }

    #[test]
    fn no_slot_is_empty_with_minimal_input() {
        // Single chromatic color + dark/light base
        let colors = vec![
            make_extracted(0.50, 0.15, 25.0, 0.60),
            make_extracted(0.10, 0.01, 0.0, 0.25),
            make_extracted(0.95, 0.01, 0.0, 0.15),
        ];

        let palette = assign_slots(&colors, ThemeMode::Dark);

        // Non-background slots should have at least some color
        for i in 1..16 {
            let c = palette.slots[i];
            assert!(
                c.r > 0 || c.g > 0 || c.b > 0,
                "slot {i} should not be completely black: {c:?}"
            );
        }
    }

    #[test]
    fn monochromatic_blue_all_accents_share_hue() {
        let palette = assign_slots_with_accent(
            &diverse_candidates(),
            ThemeMode::Dark,
            Some(AccentColor::Blue),
            AccentVariant::Vibrant,
        );

        let blue_hue = 260.0;
        for &slot in &[1, 2, 3, 4, 5, 6, 9, 10, 11, 12, 13, 14] {
            let oklch = palette.slots[slot].to_oklch();
            let dist = hue_distance(f32::from(oklch.hue), blue_hue);
            // Tolerance accounts for hue drift from sRGB gamut clamping
            // (high-chroma blues near 260° are especially prone to drift)
            assert!(
                dist < 30.0,
                "monochromatic slot {slot} hue {:.1}° should be near {blue_hue}°, distance {dist:.1}°",
                f32::from(oklch.hue)
            );
        }
    }

    #[test]
    fn monochromatic_slots_have_distinct_lightness() {
        let palette = assign_slots_with_accent(
            &diverse_candidates(),
            ThemeMode::Dark,
            Some(AccentColor::Green),
            AccentVariant::Vibrant,
        );

        let mut lightnesses: Vec<f32> = (1..=6).map(|i| palette.slots[i].to_oklch().l).collect();
        lightnesses.sort_by(|a, b| a.partial_cmp(b).unwrap());
        // Check that the range of lightness values spans at least 0.15
        let range = lightnesses.last().unwrap() - lightnesses.first().unwrap();
        assert!(
            range > 0.15,
            "monochromatic accents should have varied lightness, range was {range:.3}"
        );
    }

    #[test]
    fn empty_colors_does_not_panic() {
        let palette = assign_slots(&[], ThemeMode::Dark);
        // Should produce a valid (synthetic) palette without panicking
        for (i, color) in palette.slots.iter().enumerate() {
            let _ = color.to_hex();
            let _ = format!("slot {i}: {color}");
        }
    }
}
