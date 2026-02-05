use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::color::Color as AppColor;
use crate::pipeline::assign::AnsiPalette;

const SLOT_NAMES: [&str; 8] = ["Blk", "Red", "Grn", "Yel", "Blu", "Mag", "Cyn", "Wht"];

// ---------------------------------------------------------------------------
// PaletteWidget
// ---------------------------------------------------------------------------

/// A widget that renders the 16-color ANSI palette as an 8x2 grid of colored
/// swatches with labels. Highlights the currently selected slot.
pub struct PaletteWidget<'a> {
    palette: &'a AnsiPalette,
    selected: Option<usize>,
}

impl<'a> PaletteWidget<'a> {
    pub fn new(palette: &'a AnsiPalette, selected: Option<usize>) -> Self {
        Self { palette, selected }
    }
}

fn to_color(c: &AppColor) -> Color {
    Color::Rgb(c.r, c.g, c.b)
}

/// Choose black or white foreground for readable text on the given background.
fn contrast_fg(c: &AppColor) -> Color {
    if c.relative_luminance() > 0.4 {
        Color::Black
    } else {
        Color::White
    }
}

fn slot_name(index: usize) -> &'static str {
    SLOT_NAMES[index % 8]
}

/// Build a row of colored swatches. Each swatch is 6 chars wide with the slot
/// name centered on the colored background. Selected slot gets bold + underline.
fn build_swatch_row(
    slots: &[AppColor; 16],
    start: usize,
    selected: Option<usize>,
) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for (offset, c) in slots[start..start + 8].iter().enumerate() {
        let i = start + offset;
        let bg = to_color(c);
        let fg = contrast_fg(c);
        let is_selected = selected == Some(i);

        let label = format!("{:^6}", slot_name(i));
        let mut style = Style::default().bg(bg).fg(fg);
        if is_selected {
            style = style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        }
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

/// Build a row of slot index labels below the swatches.
fn build_index_row(start: usize, selected: Option<usize>) -> Line<'static> {
    let mut spans = vec![Span::raw("  ")];
    for i in start..start + 8 {
        let is_selected = selected == Some(i);
        let label = format!("{:^6}", i);
        let style = if is_selected {
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(" "));
    }
    Line::from(spans)
}

impl Widget for PaletteWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title("Palette");
        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = vec![
            // Normal colors (slots 0-7)
            Line::from("  Normal"),
            build_swatch_row(&self.palette.slots, 0, self.selected),
            build_index_row(0, self.selected),
            Line::from(""),
            // Bright colors (slots 8-15)
            Line::from("  Bright"),
            build_swatch_row(&self.palette.slots, 8, self.selected),
            build_index_row(8, self.selected),
        ];

        // Info line for the selected slot
        if let Some(slot) = self.selected {
            if slot < 16 {
                let color = &self.palette.slots[slot];
                let hex = color.to_hex();
                let ratio = AppColor::contrast_ratio(color, &self.palette.background);
                lines.push(Line::from(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        format!("  {}  ", slot_name(slot)),
                        Style::default().bg(to_color(color)).fg(contrast_fg(color)),
                    ),
                    Span::raw(format!(
                        "  {}:{}  {}  contrast {ratio:.1}:1",
                        slot,
                        slot_name(slot),
                        hex,
                    )),
                ]));
            }
        }

        Paragraph::new(lines).render(inner, buf);
    }
}

// ---------------------------------------------------------------------------
// PreviewWidget
// ---------------------------------------------------------------------------

/// A widget that renders a simulated terminal session using the theme colors.
pub struct PreviewWidget<'a> {
    palette: &'a AnsiPalette,
}

impl<'a> PreviewWidget<'a> {
    pub fn new(palette: &'a AnsiPalette) -> Self {
        Self { palette }
    }
}

/// Create padding to fill the rest of a line with the base style.
fn pad_line(total_width: u16, used: u16, style: Style) -> Span<'static> {
    let remaining = total_width.saturating_sub(used) as usize;
    Span::styled(" ".repeat(remaining), style)
}

impl Widget for PreviewWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title("Preview");
        let inner = block.inner(area);
        block.render(area, buf);

        let p = self.palette;
        let bg_c = to_color(&p.background);
        let fg_c = to_color(&p.foreground);
        let base = Style::default().bg(bg_c).fg(fg_c);

        let red = to_color(&p.slots[1]);
        let green = to_color(&p.slots[2]);
        let yellow = to_color(&p.slots[3]);
        let blue = to_color(&p.slots[4]);
        let magenta = to_color(&p.slots[5]);
        let cyan = to_color(&p.slots[6]);
        let bright_black = to_color(&p.slots[8]);

        let w = inner.width;

        let lines = vec![
            // Blank background line
            Line::from(Span::styled(" ".repeat(w as usize), base)),
            // Shell prompt: user@host:~/projects$ ls
            Line::from(vec![
                Span::styled("  ", base),
                Span::styled("user@host", base.fg(green)),
                Span::styled(":", base),
                Span::styled("~/projects", base.fg(blue)),
                Span::styled("$ ls", base),
                pad_line(w, 28, base),
            ]),
            // ls output — directories (blue), files (fg), config (yellow), exec (green)
            Line::from(vec![
                Span::styled("  ", base),
                Span::styled("src/", base.fg(blue)),
                Span::styled("  ", base),
                Span::styled("README.md", base.fg(fg_c)),
                Span::styled("  ", base),
                Span::styled("Cargo.toml", base.fg(yellow)),
                Span::styled("  ", base),
                Span::styled("run.sh", base.fg(green)),
                pad_line(w, 39, base),
            ]),
            // Second prompt: git diff
            Line::from(vec![
                Span::styled("  ", base),
                Span::styled("user@host", base.fg(green)),
                Span::styled(":", base),
                Span::styled("~/projects", base.fg(blue)),
                Span::styled("$ git diff", base),
                pad_line(w, 34, base),
            ]),
            // Diff deletion (red)
            Line::from(vec![
                Span::styled("  - old line removed", base.fg(red)),
                pad_line(w, 20, base),
            ]),
            // Diff addition (green)
            Line::from(vec![
                Span::styled("  + new line added", base.fg(green)),
                pad_line(w, 18, base),
            ]),
            // Comment (bright black)
            Line::from(vec![
                Span::styled("  // comment in code", base.fg(bright_black)),
                pad_line(w, 20, base),
            ]),
            // Code: fn definition (cyan keyword, magenta macro)
            Line::from(vec![
                Span::styled("  ", base),
                Span::styled("fn", base.fg(cyan)),
                Span::styled(" main() {", base),
                pad_line(w, 14, base),
            ]),
            // Code: println macro (magenta), string literal (green)
            Line::from(vec![
                Span::styled("      ", base),
                Span::styled("println!", base.fg(magenta)),
                Span::styled("(", base),
                Span::styled("\"hello\"", base.fg(green)),
                Span::styled(");", base),
                pad_line(w, 25, base),
            ]),
            // Code: let binding (cyan keyword, yellow number)
            Line::from(vec![
                Span::styled("      ", base),
                Span::styled("let", base.fg(cyan)),
                Span::styled(" x = ", base),
                Span::styled("42", base.fg(yellow)),
                Span::styled(";", base),
                pad_line(w, 17, base),
            ]),
            // Closing brace
            Line::from(vec![Span::styled("  }", base), pad_line(w, 3, base)]),
        ];

        Paragraph::new(lines).render(inner, buf);
    }
}
