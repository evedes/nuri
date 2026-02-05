pub mod widgets;

use std::io::{self, stdout};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use palette::Lab;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::backends::ghostty::{self, GhosttyBackend};
use crate::backends::ThemeBackend;
use crate::cli::ThemeMode;
use crate::pipeline::assign::{assign_slots, AnsiPalette};
use crate::pipeline::contrast::{enforce_contrast, DEFAULT_ACCENT_CONTRAST};
use crate::pipeline::extract::{extract_colors_with_seed, ExtractedColor};

use self::widgets::{PaletteWidget, PreviewWidget};

/// Input mode for the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Normal,
    NameInput,
    ConfirmQuit,
    ConfirmOverwrite,
}

/// State for the interactive TUI application.
pub struct TuiApp {
    pub palette: AnsiPalette,
    pub extracted_colors: Vec<ExtractedColor>,
    pub image_path: PathBuf,
    pub mode: ThemeMode,
    pub selected_slot: Option<usize>,
    pub theme_name: String,
    pub show_help: bool,
    pub dirty: bool,
    pub status_message: Option<String>,
    input_mode: InputMode,
    name_input_buf: String,
    pixels: Vec<Lab>,
    k: usize,
    seed: u64,
}

impl TuiApp {
    pub fn new(
        palette: AnsiPalette,
        extracted_colors: Vec<ExtractedColor>,
        image_path: PathBuf,
        mode: ThemeMode,
        theme_name: String,
        pixels: Vec<Lab>,
        k: usize,
    ) -> Self {
        Self {
            palette,
            extracted_colors,
            image_path,
            mode,
            selected_slot: None,
            theme_name: theme_name.clone(),
            show_help: false,
            dirty: false,
            status_message: None,
            input_mode: InputMode::Normal,
            name_input_buf: theme_name,
            pixels,
            k,
            seed: 42,
        }
    }
}

/// Launch the TUI application.
pub fn run(mut app: TuiApp) -> Result<()> {
    enable_raw_mode()?;
    stdout().execute(EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let result = run_event_loop(&mut terminal, &mut app);

    // Always restore terminal, even on error
    disable_raw_mode()?;
    stdout().execute(LeaveAlternateScreen)?;

    result
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiApp,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match app.input_mode {
                        InputMode::NameInput => handle_name_input(app, key.code),
                        InputMode::ConfirmQuit => match key.code {
                            KeyCode::Char('y') => return Ok(()),
                            _ => app.input_mode = InputMode::Normal,
                        },
                        InputMode::ConfirmOverwrite => {
                            handle_confirm_overwrite(app, key.code);
                        }
                        InputMode::Normal => {
                            if handle_normal_input(app, key.code) {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
    }
}

fn handle_name_input(app: &mut TuiApp, code: KeyCode) {
    match code {
        KeyCode::Enter => {
            if let Err(e) = try_save(app) {
                app.status_message = Some(format!("Error: {e}"));
                app.input_mode = InputMode::Normal;
            }
        }
        KeyCode::Esc => app.input_mode = InputMode::Normal,
        KeyCode::Backspace => {
            app.name_input_buf.pop();
        }
        KeyCode::Char(c) => app.name_input_buf.push(c),
        _ => {}
    }
}

fn handle_confirm_overwrite(app: &mut TuiApp, code: KeyCode) {
    match code {
        KeyCode::Char('y') => {
            let name = app.name_input_buf.trim().to_string();
            if let Err(e) = do_save(app, &name) {
                app.status_message = Some(format!("Error: {e}"));
            }
            app.input_mode = InputMode::Normal;
        }
        _ => app.input_mode = InputMode::Normal,
    }
}

/// Handle key input in normal mode. Returns true if the app should quit.
fn handle_normal_input(app: &mut TuiApp, code: KeyCode) -> bool {
    app.status_message = None;
    match code {
        KeyCode::Char('q') => {
            if app.dirty {
                app.input_mode = InputMode::ConfirmQuit;
            } else {
                return true;
            }
        }
        KeyCode::Char('?') => app.show_help = !app.show_help,
        KeyCode::Tab => cycle_slot(app),
        KeyCode::BackTab => cycle_slot_reverse(app),
        KeyCode::Char(c @ '1'..='6') => {
            app.selected_slot = Some((c as u8 - b'0') as usize);
        }
        KeyCode::Esc => {
            if app.show_help {
                app.show_help = false;
            } else {
                app.selected_slot = None;
            }
        }
        KeyCode::Char('d') => switch_mode(app, ThemeMode::Dark),
        KeyCode::Char('l') => switch_mode(app, ThemeMode::Light),
        KeyCode::Char('r') => regenerate(app),
        KeyCode::Char('+') | KeyCode::Char('=') => adjust_lightness(app, 0.02),
        KeyCode::Char('-') => adjust_lightness(app, -0.02),
        KeyCode::Char('s') => adjust_chroma(app, -0.02),
        KeyCode::Char('S') => adjust_chroma(app, 0.02),
        KeyCode::Left => cycle_candidate(app, false),
        KeyCode::Right => cycle_candidate(app, true),
        KeyCode::Enter => {
            app.name_input_buf.clone_from(&app.theme_name);
            app.input_mode = InputMode::NameInput;
        }
        _ => {}
    }
    false
}

// ---------------------------------------------------------------------------
// Slot navigation
// ---------------------------------------------------------------------------

fn cycle_slot(app: &mut TuiApp) {
    app.selected_slot = Some(match app.selected_slot {
        None | Some(15) => 0,
        Some(n) => n + 1,
    });
}

fn cycle_slot_reverse(app: &mut TuiApp) {
    app.selected_slot = Some(match app.selected_slot {
        None | Some(0) => 15,
        Some(n) => n - 1,
    });
}

// ---------------------------------------------------------------------------
// Pipeline re-run helpers
// ---------------------------------------------------------------------------

fn switch_mode(app: &mut TuiApp, mode: ThemeMode) {
    if app.mode == mode {
        return;
    }
    app.mode = mode;
    app.palette = assign_slots(&app.extracted_colors, app.mode);
    enforce_contrast(&mut app.palette, DEFAULT_ACCENT_CONTRAST);
    app.dirty = true;
    app.selected_slot = None;
    app.status_message = Some(format!("Switched to {mode:?} mode"));
}

fn regenerate(app: &mut TuiApp) {
    app.seed = app.seed.wrapping_add(1);
    app.extracted_colors = extract_colors_with_seed(&app.pixels, app.k, app.seed);
    app.palette = assign_slots(&app.extracted_colors, app.mode);
    enforce_contrast(&mut app.palette, DEFAULT_ACCENT_CONTRAST);
    app.dirty = true;
    app.selected_slot = None;
    app.status_message = Some("Regenerated palette".to_string());
}

fn adjust_lightness(app: &mut TuiApp, delta: f32) {
    if let Some(slot) = app.selected_slot {
        if slot < 16 {
            app.palette.slots[slot] = app.palette.slots[slot].adjust_lightness(delta);
            recompute_after_tweak(app);
        }
    }
}

fn adjust_chroma(app: &mut TuiApp, delta: f32) {
    if let Some(slot) = app.selected_slot {
        if slot < 16 {
            app.palette.slots[slot] = app.palette.slots[slot].adjust_chroma(delta);
            recompute_after_tweak(app);
        }
    }
}

/// Cycle the selected slot through extracted candidate colors.
fn cycle_candidate(app: &mut TuiApp, forward: bool) {
    let slot = match app.selected_slot {
        Some(s) if s < 16 => s,
        _ => return,
    };
    if app.extracted_colors.is_empty() {
        return;
    }

    let current = app.palette.slots[slot];
    let n = app.extracted_colors.len();

    // Find the extracted color closest to the current slot color (by ΔE² in Lab)
    let closest_idx = app
        .extracted_colors
        .iter()
        .enumerate()
        .min_by_key(|(_, ec)| {
            let lab1 = current.to_lab();
            let lab2 = ec.color.to_lab();
            let de_sq =
                (lab1.l - lab2.l).powi(2) + (lab1.a - lab2.a).powi(2) + (lab1.b - lab2.b).powi(2);
            (de_sq * 1000.0) as i64
        })
        .map(|(i, _)| i)
        .unwrap_or(0);

    let next_idx = if forward {
        (closest_idx + 1) % n
    } else {
        (closest_idx + n - 1) % n
    };

    app.palette.slots[slot] = app.extracted_colors[next_idx].color;
    recompute_after_tweak(app);
}

/// Sync special colors from base slots and re-enforce contrast.
fn recompute_after_tweak(app: &mut TuiApp) {
    app.palette.background = app.palette.slots[0];
    app.palette.cursor_text = app.palette.background;
    enforce_contrast(&mut app.palette, DEFAULT_ACCENT_CONTRAST);
    app.dirty = true;
}

// ---------------------------------------------------------------------------
// Save helpers
// ---------------------------------------------------------------------------

fn try_save(app: &mut TuiApp) -> Result<()> {
    let name = app.name_input_buf.trim().to_string();
    if name.is_empty() {
        app.status_message = Some("Theme name cannot be empty".to_string());
        app.input_mode = InputMode::Normal;
        return Ok(());
    }

    let path = ghostty::theme_path(&name)?;
    if path.exists() {
        app.input_mode = InputMode::ConfirmOverwrite;
        return Ok(());
    }

    do_save(app, &name)
}

fn do_save(app: &mut TuiApp, name: &str) -> Result<()> {
    let backend = GhosttyBackend;
    backend.install(&app.palette, name)?;
    app.theme_name = name.to_string();
    app.dirty = false;
    app.status_message = Some(format!("Saved theme '{name}'"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw(f: &mut Frame, app: &TuiApp) {
    // Main layout: top section, preview, status bar
    let main_layout = Layout::vertical([
        Constraint::Min(10),
        Constraint::Percentage(40),
        Constraint::Length(1),
    ])
    .split(f.area());

    // Top: image (30%) | palette (70%)
    let top_layout = Layout::horizontal([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(main_layout[0]);

    draw_image_pane(f, app, top_layout[0]);
    draw_palette_pane(f, app, top_layout[1]);

    let preview = PreviewWidget::new(&app.palette);
    f.render_widget(preview, main_layout[1]);

    draw_status_bar(f, app, main_layout[2]);

    // Overlays
    match app.input_mode {
        InputMode::Normal => {
            if app.show_help {
                draw_help_overlay(f);
            }
        }
        InputMode::NameInput => draw_name_input_overlay(f, app),
        InputMode::ConfirmQuit => draw_confirm_quit_overlay(f),
        InputMode::ConfirmOverwrite => {
            draw_confirm_overwrite_overlay(f, &app.name_input_buf);
        }
    }
}

fn draw_image_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let block = Block::bordered().title("Image");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let mut lines = vec![
        Line::from(""),
        Line::from(format!("  {}", app.image_path.display())),
        Line::from(""),
        Line::from(format!("  Mode: {:?}", app.mode)),
        Line::from(format!("  Theme: {}", app.theme_name)),
        Line::from(format!("  Colors: {}", app.extracted_colors.len())),
        Line::from(""),
    ];

    // Show extracted color swatches
    let mut swatch_spans = vec![Span::raw("  ")];
    for ec in app.extracted_colors.iter().take(12) {
        let c = &ec.color;
        let bg = Color::Rgb(c.r, c.g, c.b);
        swatch_spans.push(Span::styled("  ", Style::default().bg(bg)));
    }
    lines.push(Line::from(swatch_spans));

    if app.dirty {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  [Modified]",
            Style::default().fg(Color::Yellow),
        )));
    }

    f.render_widget(Paragraph::new(lines), inner);
}

fn draw_palette_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let widget = PaletteWidget::new(&app.palette, app.selected_slot);
    f.render_widget(widget, area);
}

fn draw_status_bar(f: &mut Frame, app: &TuiApp, area: Rect) {
    let text = if let Some(msg) = &app.status_message {
        format!(" {msg}")
    } else if app.selected_slot.is_some() {
        " +/-: Lightness | s/S: Chroma | Left/Right: Cycle | Enter: Save | q: Quit".to_string()
    } else {
        " d/l: Mode | r: Regen | Tab: Cycle | 1-6: Select | Enter: Save | ?: Help | q: Quit"
            .to_string()
    };
    let bar = Paragraph::new(text).style(
        Style::default()
            .fg(Color::DarkGray)
            .bg(Color::Rgb(20, 20, 20)),
    );
    f.render_widget(bar, area);
}

fn draw_help_overlay(f: &mut Frame) {
    let area = centered_rect(60, 70, f.area());
    let lines = vec![
        Line::from(""),
        Line::from("  Keybindings:"),
        Line::from(""),
        Line::from("  q             Quit (confirm if unsaved)"),
        Line::from("  ?             Toggle this help"),
        Line::from("  Tab           Next slot"),
        Line::from("  Shift+Tab     Previous slot"),
        Line::from("  1-6           Select accent slot"),
        Line::from("  Esc           Deselect / close"),
        Line::from("  d / l         Switch to dark / light mode"),
        Line::from("  r             Regenerate palette (new seed)"),
        Line::from("  Enter         Save theme"),
        Line::from(""),
        Line::from("  When a slot is selected:"),
        Line::from("  + / -         Adjust lightness"),
        Line::from("  s / S         Adjust chroma"),
        Line::from("  Left / Right  Cycle through extracted colors"),
        Line::from(""),
        Line::from("  Press ? or Esc to close"),
    ];
    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Help "))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn draw_name_input_overlay(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(50, 25, f.area());
    let lines = vec![
        Line::from(""),
        Line::from("  Save theme as:"),
        Line::from(""),
        Line::from(vec![
            Span::raw("  > "),
            Span::styled(
                app.name_input_buf.clone(),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("\u{2588}", Style::default().fg(Color::Yellow)),
        ]),
        Line::from(""),
        Line::from("  Enter: Save | Esc: Cancel"),
    ];
    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Save Theme "))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn draw_confirm_quit_overlay(f: &mut Frame) {
    let area = centered_rect(40, 20, f.area());
    let lines = vec![
        Line::from(""),
        Line::from("  Unsaved changes!"),
        Line::from(""),
        Line::from("  Quit without saving?"),
        Line::from(""),
        Line::from("  y: Yes | any other key: No"),
    ];
    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Confirm Quit "))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn draw_confirm_overwrite_overlay(f: &mut Frame, name: &str) {
    let area = centered_rect(50, 20, f.area());
    let lines = vec![
        Line::from(""),
        Line::from(format!("  Theme '{name}' already exists.")),
        Line::from(""),
        Line::from("  Overwrite?"),
        Line::from(""),
        Line::from("  y: Yes | any other key: No"),
    ];
    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Confirm Overwrite "))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage((100 - percent_y) / 2),
        Constraint::Percentage(percent_y),
        Constraint::Percentage((100 - percent_y) / 2),
    ])
    .split(r);
    Layout::horizontal([
        Constraint::Percentage((100 - percent_x) / 2),
        Constraint::Percentage(percent_x),
        Constraint::Percentage((100 - percent_x) / 2),
    ])
    .split(v[1])[1]
}
