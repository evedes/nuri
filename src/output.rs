//! Non-interactive output formats — the "escape hatch".
//!
//! These render the fully resolved palette to stdout so external tools can
//! template apps nuri doesn't natively support, without nuri owning a template
//! DSL. The output is target-agnostic: it is the raw palette, not a Ghostty /
//! Zellij / Neovim theme.

use crate::cli::{OutputFormat, ThemeMode};
use crate::pipeline::assign::AnsiPalette;

/// Render the resolved palette in the requested non-interactive format.
pub fn render(palette: &AnsiPalette, name: &str, mode: ThemeMode, format: OutputFormat) -> String {
    match format {
        OutputFormat::Json => to_json(palette, name, mode),
    }
}

/// Serialize the palette as a JSON object.
///
/// Shape:
/// ```json
/// {
///   "name": "sunset",
///   "mode": "dark",
///   "special": { "background": "#...", "foreground": "#...", ... },
///   "palette": ["#...", ... 16 entries ...]
/// }
/// ```
fn to_json(p: &AnsiPalette, name: &str, mode: ThemeMode) -> String {
    let mode_str = match mode {
        ThemeMode::Dark => "dark",
        ThemeMode::Light => "light",
    };

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"name\": \"{}\",\n", escape_json(name)));
    out.push_str(&format!("  \"mode\": \"{mode_str}\",\n"));

    out.push_str("  \"special\": {\n");
    out.push_str(&format!(
        "    \"background\": \"{}\",\n",
        p.background.to_hex()
    ));
    out.push_str(&format!(
        "    \"foreground\": \"{}\",\n",
        p.foreground.to_hex()
    ));
    out.push_str(&format!(
        "    \"cursor_color\": \"{}\",\n",
        p.cursor_color.to_hex()
    ));
    out.push_str(&format!(
        "    \"cursor_text\": \"{}\",\n",
        p.cursor_text.to_hex()
    ));
    out.push_str(&format!(
        "    \"selection_background\": \"{}\",\n",
        p.selection_bg.to_hex()
    ));
    out.push_str(&format!(
        "    \"selection_foreground\": \"{}\"\n",
        p.selection_fg.to_hex()
    ));
    out.push_str("  },\n");

    out.push_str("  \"palette\": [\n");
    for (i, color) in p.slots.iter().enumerate() {
        let comma = if i + 1 < p.slots.len() { "," } else { "" };
        out.push_str(&format!("    \"{}\"{comma}\n", color.to_hex()));
    }
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

/// Escape a string for inclusion in a JSON string literal.
fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::Color;
    use palette::Oklch;

    fn sample_palette() -> AnsiPalette {
        use crate::cli::AccentVariant;
        use crate::pipeline::assign::assign_slots_with_accent;
        use crate::pipeline::extract::ExtractedColor;

        let make = |l, c, h, w| ExtractedColor {
            color: Color::from_oklch(Oklch::new(l, c, h)),
            weight: w,
        };
        let colors = vec![
            make(0.60, 0.20, 25.0, 0.2),
            make(0.55, 0.20, 260.0, 0.2),
            make(0.10, 0.01, 0.0, 0.3),
            make(0.95, 0.01, 0.0, 0.3),
        ];
        assign_slots_with_accent(&colors, ThemeMode::Dark, None, AccentVariant::Vibrant)
    }

    #[test]
    fn json_has_all_sections() {
        let json = to_json(&sample_palette(), "sunset", ThemeMode::Dark);
        assert!(json.contains("\"name\": \"sunset\""));
        assert!(json.contains("\"mode\": \"dark\""));
        assert!(json.contains("\"background\""));
        assert!(json.contains("\"selection_foreground\""));
        assert!(json.contains("\"palette\""));
    }

    #[test]
    fn json_has_16_palette_entries() {
        let json = to_json(&sample_palette(), "t", ThemeMode::Dark);
        // Count hex strings inside the palette array section.
        let array = json.split("\"palette\": [").nth(1).unwrap();
        let count = array.matches('#').count();
        assert_eq!(count, 16, "expected 16 palette colors, got {count}");
    }

    #[test]
    fn json_light_mode_label() {
        let json = to_json(&sample_palette(), "t", ThemeMode::Light);
        assert!(json.contains("\"mode\": \"light\""));
    }

    #[test]
    fn json_balanced_braces() {
        let json = to_json(&sample_palette(), "t", ThemeMode::Dark);
        let opens = json.matches('{').count();
        let closes = json.matches('}').count();
        assert_eq!(opens, closes, "unbalanced braces");
    }

    #[test]
    fn escape_handles_quotes_and_backslashes() {
        assert_eq!(escape_json(r#"a"b\c"#), r#"a\"b\\c"#);
        assert_eq!(escape_json("line\nbreak"), "line\\nbreak");
    }

    #[test]
    fn json_with_special_name_is_escapable() {
        let json = to_json(&sample_palette(), "we\"ird", ThemeMode::Dark);
        assert!(json.contains(r#""name": "we\"ird""#));
    }
}
