# NURI

> 塗り (*nuri*) — Japanese for "to paint" or "to coat"

Generate color themes from wallpaper images. Supports [Ghostty](https://ghostty.org/), [Zellij](https://zellij.dev/), and [Neovim](https://neovim.io/) backends.


![nuri-generated theme applied to Ghostty, Zellij, and Neovim](public/assets/desktop-202602060052.png)

nuri extracts dominant colors from an image using K-means clustering, maps them to ANSI palette slots via perceptual hue matching, enforces WCAG contrast minimums, and outputs a ready-to-use theme file.

## Examples

See [EXAMPLES.md](EXAMPLES.md) for more screenshots of nuri-generated themes in action.

## How it works

```
Image → resize 256x256 → K-means (LAB, K=16) → deduplicate → detect dark/light
      → hue-based ANSI slot assignment (Oklch) → bright variants → derive special colors
      → WCAG contrast enforcement → theme file
```

- **K-means in LAB space** for perceptually diverse palette extraction
- **Oklch color space** for all lightness, chroma, and hue adjustments
- **WCAG 2.0 contrast enforcement**: 4.5:1 for accents, 7:1 for foreground, 3:1 for bright-black
- **Auto dark/light detection** based on image luminance (overridable)

## Installation

```bash
cargo install --path .
```

## Usage

```bash
# Launch with default settings
nuri ~/wallpapers/sunset.jpg

# Target specific backend(s)
nuri ~/wallpapers/sunset.jpg --target zellij
nuri ~/wallpapers/sunset.jpg --target ghostty,zellij,neovim

# Force light mode
nuri ~/wallpapers/sunset.jpg --mode light

# Use a monochromatic accent palette
nuri ~/wallpapers/sunset.jpg --accent blue

# Print the resolved palette as JSON and exit (no TUI) — pipe it into your own
# templates to theme apps nuri doesn't natively support
nuri ~/wallpapers/sunset.jpg --format json
```

Grayscale or single-hue wallpapers are detected automatically: instead of
inventing a saturated rainbow, nuri produces a restrained palette whose accents
track the ANSI hue positions but stay near-neutral, distinguished by lightness.

nuri launches an interactive TUI for previewing and tweaking the generated palette before saving. Keybindings:

| Key | Action |
|-----|--------|
| `d` / `l` | Toggle dark/light mode |
| `r` | Regenerate palette (new K-means seed) |
| `Tab` / `Shift+Tab` | Cycle through palette slots |
| `1`-`6` | Select accent slot |
| `+` / `-` | Adjust lightness (selected slot) |
| `s` / `S` | Adjust chroma (selected slot) |
| `Left` / `Right` | Cycle extracted colors (selected slot) |
| `Enter` | Save theme |
| `q` | Quit |
| `?` | Help |

### All options

```
nuri [OPTIONS] <IMAGE>

Arguments:
  <IMAGE>                            Path to the input image

Options:
  -n, --name <NAME>                  Theme name (defaults to image filename)
  -m, --mode <MODE>                  Force dark or light [values: dark, light]
  -t, --target <TARGET>              Backend(s), comma-separated [values: ghostty, zellij, neovim]
  -k, --colors <N>                   K-means clusters [default: 16]
      --min-contrast <RATIO>         Minimum accent contrast ratio [default: 4.5]
      --accent <COLOR>               Monochromatic palette [values: blue, green, yellow, red, purple, gray]
      --format <FORMAT>              Print resolved palette to stdout and exit [values: json]
```

## Development

```bash
cargo build                  # Build
cargo test                   # Run tests
cargo clippy                 # Lint
cargo fmt --check            # Check formatting
./check.sh                   # Run all checks (fmt, clippy, test, build)
```

## Tech stack

| Crate | Purpose |
|-------|---------|
| [clap](https://crates.io/crates/clap) | CLI argument parsing |
| [image](https://crates.io/crates/image) | Image loading and resizing |
| [kmeans-colors](https://crates.io/crates/kmeans-colors) | K-means clustering for color extraction |
| [palette](https://crates.io/crates/palette) | Color space conversions (sRGB, LAB, Oklch) |
| [ratatui](https://crates.io/crates/ratatui) | Terminal UI framework |
| [crossterm](https://crates.io/crates/crossterm) | Terminal backend for ratatui |
| [anyhow](https://crates.io/crates/anyhow) | Error handling |


## License

MIT
