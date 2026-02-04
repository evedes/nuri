# CLAUDE.md — ghostty-themer

## Project Overview

ghostty-themer is a Rust CLI/TUI app that generates Ghostty terminal color themes from wallpaper images. See `product-definition/PRD.md` for full requirements and `product-definition/TICKETS.md` for implementation tickets.

## Agent Guidelines

- Only commit or push if I strictly tell you to do so. Never do it on your own. If you want to do it ALWAYS ask first.
- Always use the best subagents for the job
- When a task would benefit from a subagent, custom command, or reusable skill, suggest creating one before proceeding.
- When committing a ticket, mark it as done in the @TICKETS.md file.
- When committing a ticket update all the docs with the new ticket features, fixes, enhancements.


## Build & Run

```bash
cargo build                  # Build
cargo run -- <image> [opts]  # Run CLI mode
cargo run -- <image> --tui   # Run TUI mode
cargo test                   # Run all tests
cargo clippy                 # Lint
cargo fmt --check            # Check formatting
```

## Project Structure

```
src/
  main.rs              # Entry point, CLI dispatch
  cli.rs               # Clap arg definitions
  pipeline/
    mod.rs
    extract.rs         # Image loading, K-means color extraction
    detect.rs          # Dark/light mode auto-detection
    assign.rs          # Hue-based ANSI slot assignment (Oklch)
    contrast.rs        # WCAG contrast enforcement
  theme.rs             # GhosttyTheme struct, serialization, install
  tui/
    mod.rs             # TUI app loop, event handling
    widgets.rs         # Custom ratatui widgets (palette, preview)
tests/
  fixtures/            # Test images
  snapshots/           # Expected theme output snapshots
product-definition/
  PRD.md               # Product requirements
  TICKETS.md           # Implementation tickets
```

## Code Conventions

- **Rust edition**: 2021
- **Error handling**: Use `anyhow::Result` for application errors. Use `thiserror` if defining library-style error enums. No `.unwrap()` in non-test code.
- **Color space rule**: All lightness/saturation/hue adjustments operate in **Oklch** space, never in RGB or HSL. Use `palette` crate for conversions.
- **K-means runs in LAB space** via `kmeans-colors`. Do not run K-means in RGB.
- **Formatting**: Run `cargo fmt` before committing. Use default rustfmt settings.
- **Linting**: Code must pass `cargo clippy` with no warnings.
- **Tests**: Every pipeline module has unit tests in-file (`#[cfg(test)] mod tests`). Integration tests go in `tests/`. Use `#[test]` — no external test runner.
- **No unsafe code** — there is no reason to need it in this project.
- **Dependencies**: Prefer well-maintained crates. Core deps are `clap`, `image`, `kmeans-colors`, `palette`, `ratatui`, `crossterm`, `anyhow`. Do not add dependencies without justification.

## Ghostty Theme Format

Theme files are plain key-value, placed in `~/.config/ghostty/themes/<name>` (no file extension).

```
background = #RRGGBB
foreground = #RRGGBB
cursor-color = #RRGGBB
cursor-text = #RRGGBB
selection-background = #RRGGBB
selection-foreground = #RRGGBB
palette = 0=#RRGGBB
palette = 1=#RRGGBB
...
palette = 15=#RRGGBB
```

**Critical**: No whitespace around the inner `=` in palette lines (`palette = 0=#RRGGBB`, not `palette = 0 = #RRGGBB`). Hex values are lowercase.

## Color Pipeline Summary

Image → resize 256x256 → K-means (LAB, K=16) → deduplicate (ΔE<5) → detect dark/light → hue-based slot assignment (Oklch) → bright variants (+0.12 L) → derive special colors → WCAG contrast enforcement (4.5:1 accents, 7:1 foreground, 3:1 bright-black) → serialize.

## Contrast Requirements

- Accent colors (slots 1–6, 9–14) vs background: **≥ 4.5:1**
- Foreground vs background: **≥ 7:1**
- Bright black (slot 8) vs background: **≥ 3:1**
- Use WCAG 2.0 relative luminance formula. Adjust only Oklch lightness to fix contrast — preserve hue and chroma.

## Pre-Commit Checklist

Run **before every commit**. All checks must pass — do not skip or `--no-verify`.

```bash
./check.sh
```

This runs formatting, linting, tests, and build in sequence (`set -euo pipefail` — stops on first failure). Fix any issues and re-run before committing.

## Commit Style

- Use conventional commits: `feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`
- Keep messages concise, imperative mood: "feat: add contrast enforcement" not "Added contrast enforcement"
- Reference ticket numbers where relevant: "feat: add K-means extraction (ticket #4)"
