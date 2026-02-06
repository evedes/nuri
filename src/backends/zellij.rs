use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use palette::Oklch;

use crate::color::Color;
use crate::pipeline::assign::AnsiPalette;

use super::ThemeBackend;

/// Zellij terminal multiplexer theme backend (KDL format).
pub struct ZellijBackend;

impl ThemeBackend for ZellijBackend {
    fn name(&self) -> &str {
        "Zellij"
    }

    fn serialize(&self, palette: &AnsiPalette, theme_name: &str) -> String {
        let orange = derive_orange(palette);

        let mut out = String::new();
        out.push_str("themes {\n");
        out.push_str(&format!("    {} {{\n", theme_name));
        out.push_str(&format!("        fg \"{}\"\n", palette.foreground.to_hex()));
        out.push_str(&format!("        bg \"{}\"\n", palette.background.to_hex()));
        out.push_str(&format!(
            "        black \"{}\"\n",
            palette.slots[0].to_hex()
        ));
        out.push_str(&format!("        red \"{}\"\n", palette.slots[1].to_hex()));
        out.push_str(&format!(
            "        green \"{}\"\n",
            palette.slots[2].to_hex()
        ));
        out.push_str(&format!(
            "        yellow \"{}\"\n",
            palette.slots[3].to_hex()
        ));
        out.push_str(&format!("        blue \"{}\"\n", palette.slots[4].to_hex()));
        out.push_str(&format!(
            "        magenta \"{}\"\n",
            palette.slots[5].to_hex()
        ));
        out.push_str(&format!("        cyan \"{}\"\n", palette.slots[6].to_hex()));
        out.push_str(&format!(
            "        white \"{}\"\n",
            palette.slots[7].to_hex()
        ));
        out.push_str(&format!("        orange \"{}\"\n", orange.to_hex()));
        out.push_str("    }\n");
        out.push_str("}\n");

        out
    }

    fn install(&self, palette: &AnsiPalette, theme_name: &str) -> Result<PathBuf> {
        let dir = themes_dir()?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("failed to create themes directory: {}", dir.display()))?;

        let path = dir.join(format!("{}.kdl", theme_name));
        self.write_to(palette, theme_name, &path)?;
        Ok(path)
    }

    fn write_to(&self, palette: &AnsiPalette, theme_name: &str, path: &Path) -> Result<()> {
        let content = self.serialize(palette, theme_name);
        std::fs::write(path, content)
            .with_context(|| format!("failed to write theme to {}", path.display()))?;
        Ok(())
    }
}

/// Derive the Zellij-specific "orange" color by interpolating between
/// slot 1 (red) and slot 3 (yellow) in Oklch space, targeting hue ~55°.
fn derive_orange(palette: &AnsiPalette) -> Color {
    let red = palette.slots[1].to_oklch();
    let yellow = palette.slots[3].to_oklch();

    let l = (red.l + yellow.l) / 2.0;
    let chroma = (red.chroma + yellow.chroma) / 2.0;
    let hue = 55.0;

    Color::from_oklch(Oklch::new(l, chroma, hue))
}

/// Resolve the Zellij themes directory.
fn themes_dir() -> Result<PathBuf> {
    let config_home = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
            PathBuf::from(home).join(".config")
        });
    Ok(config_home.join("zellij").join("themes"))
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

    fn test_palette() -> AnsiPalette {
        let colors = vec![
            make_extracted(0.60, 0.20, 25.0, 0.12),
            make_extracted(0.60, 0.20, 145.0, 0.12),
            make_extracted(0.70, 0.20, 90.0, 0.12),
            make_extracted(0.55, 0.20, 260.0, 0.12),
            make_extracted(0.60, 0.20, 325.0, 0.12),
            make_extracted(0.65, 0.20, 195.0, 0.10),
            make_extracted(0.10, 0.01, 0.0, 0.15),
            make_extracted(0.95, 0.01, 0.0, 0.15),
        ];
        assign_slots(&colors, ThemeMode::Dark)
    }

    #[test]
    fn serialization_contains_all_color_keys() {
        let backend = ZellijBackend;
        let output = backend.serialize(&test_palette(), "test");

        let keys = [
            "fg", "bg", "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
            "orange",
        ];
        for key in &keys {
            assert!(
                output.contains(&format!("{} \"#", key)),
                "output should contain key '{key}'"
            );
        }
    }

    #[test]
    fn theme_name_is_embedded() {
        let backend = ZellijBackend;
        let output = backend.serialize(&test_palette(), "my-wallpaper");
        assert!(output.contains("my-wallpaper {"));
    }

    #[test]
    fn hex_values_are_lowercase_and_quoted() {
        let backend = ZellijBackend;
        let output = backend.serialize(&test_palette(), "test");

        for line in output.lines() {
            if let Some(start) = line.find("\"#") {
                let hex_start = start + 1;
                let hex = &line[hex_start..hex_start + 7];
                assert_eq!(hex, &hex.to_lowercase(), "hex not lowercase: '{hex}'");
                // Check it's quoted
                assert_eq!(&line[start..start + 1], "\"", "hex should be double-quoted");
                assert_eq!(
                    &line[hex_start + 7..hex_start + 8],
                    "\"",
                    "hex should have closing quote"
                );
            }
        }
    }

    #[test]
    fn orange_is_not_black() {
        let orange = derive_orange(&test_palette());
        assert!(
            orange.r > 0 || orange.g > 0 || orange.b > 0,
            "orange should not be black: {orange}"
        );
    }

    #[test]
    fn orange_hue_is_between_red_and_yellow() {
        let palette = test_palette();
        let orange = derive_orange(&palette);
        let oklch = orange.to_oklch();
        let hue: f32 = oklch.hue.into();
        // Orange target hue is 55°, allow some tolerance for gamut clamping
        assert!(
            (hue - 55.0).abs() < 20.0,
            "orange hue should be near 55°, got {hue:.1}°"
        );
    }

    #[test]
    fn output_has_correct_kdl_structure() {
        let backend = ZellijBackend;
        let output = backend.serialize(&test_palette(), "test");

        assert!(output.starts_with("themes {"));
        assert!(output.contains("    test {"));
        assert!(output.ends_with("}\n"));

        // Count nesting: 11 color lines at 8-space indent
        let color_lines: Vec<&str> = output
            .lines()
            .filter(|l| l.starts_with("        "))
            .collect();
        assert_eq!(
            color_lines.len(),
            11,
            "expected 11 color lines, got {}",
            color_lines.len()
        );
    }

    #[test]
    fn write_to_creates_file() {
        let backend = ZellijBackend;
        let palette = test_palette();
        let dir = std::env::temp_dir().join("nuri-test-zellij-backend");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test-theme.kdl");

        backend.write_to(&palette, "test-theme", &path).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content, backend.serialize(&palette, "test-theme"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn install_creates_correct_path() {
        let temp_dir = std::env::temp_dir().join("nuri-test-zellij-install");
        std::env::set_var("XDG_CONFIG_HOME", &temp_dir);

        let backend = ZellijBackend;
        let palette = test_palette();
        let result = backend.install(&palette, "my-theme").unwrap();

        let expected_path = temp_dir.join("zellij").join("themes").join("my-theme.kdl");
        assert_eq!(result, expected_path);
        assert!(expected_path.exists());

        let content = std::fs::read_to_string(&expected_path).unwrap();
        assert_eq!(content, backend.serialize(&palette, "my-theme"));

        std::fs::remove_dir_all(&temp_dir).unwrap();
        std::env::remove_var("XDG_CONFIG_HOME");
    }
}
