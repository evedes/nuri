use ratatui::prelude::*;
use ratatui::widgets::{Block, Paragraph, Widget};

use crate::color::Color as AppColor;
use crate::pipeline::assign::AnsiPalette;

const SLOT_NAMES: [&str; 8] = ["Blk", "Red", "Grn", "Yel", "Blu", "Mag", "Cyn", "Wht"];

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
