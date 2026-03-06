pub mod widgets;

use std::io::{self, stdout};
use std::path::{Path, PathBuf};
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

use crate::backends::{get_backend, Target};
use crate::cli::{AccentColor, AccentVariant, ThemeMode};
use crate::pipeline::assign::{assign_slots_with_accent, preview_monochromatic, AnsiPalette};
use crate::pipeline::contrast::{enforce_contrast, DEFAULT_ACCENT_CONTRAST};
use crate::pipeline::extract::{extract_colors_with_seed, ExtractedColor};

use self::widgets::{PaletteWidget, PreviewWidget};

/// Input mode for the TUI.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputMode {
    Normal,
    AccentSelect,
    BackendSelect,
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
    /// Targets passed via --target CLI flag (empty = show picker).
    cli_targets: Vec<Target>,
    /// Backend selection state for the picker popup.
    selected_backends: [bool; 3],
    /// Monochromatic accent color (None = normal multi-hue mode).
    accent: Option<AccentColor>,
    /// Style variant for accent mode.
    accent_variant: AccentVariant,
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
            name_input_buf: format!("~/{theme_name}"),
            pixels,
            k,
            seed: 42,
            cli_targets: Vec::new(),
            selected_backends: [true, false, false],
            accent: None,
            accent_variant: AccentVariant::Vibrant,
        }
    }

    /// Set targets from the CLI --target flag.
    pub fn set_targets(&mut self, targets: Vec<Target>) {
        self.cli_targets = targets;
    }

    /// Set the monochromatic accent color.
    pub fn set_accent(&mut self, accent: Option<AccentColor>) {
        self.accent = accent;
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
                        InputMode::AccentSelect => {
                            handle_accent_select(app, key.code);
                        }
                        InputMode::BackendSelect => {
                            handle_backend_select(app, key.code);
                        }
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
            if let Err(e) = do_save(app) {
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
        KeyCode::Char('a') => app.input_mode = InputMode::AccentSelect,
        KeyCode::Char('r') => regenerate(app),
        KeyCode::Char('+') | KeyCode::Char('=') => adjust_lightness(app, 0.02),
        KeyCode::Char('-') => adjust_lightness(app, -0.02),
        KeyCode::Char('s') => adjust_chroma(app, -0.02),
        KeyCode::Char('S') => adjust_chroma(app, 0.02),
        KeyCode::Left => cycle_candidate(app, false),
        KeyCode::Right => cycle_candidate(app, true),
        KeyCode::Enter => {
            if app.cli_targets.is_empty() {
                // No --target specified: show backend picker
                app.selected_backends = [true, false, false];
                app.input_mode = InputMode::BackendSelect;
            } else {
                // --target specified: skip picker, go straight to name input
                app.name_input_buf = format!("~/{}", app.theme_name);
                app.input_mode = InputMode::NameInput;
            }
        }
        _ => {}
    }
    false
}

fn handle_backend_select(app: &mut TuiApp, code: KeyCode) {
    match code {
        KeyCode::Char('g') => app.selected_backends[0] = !app.selected_backends[0],
        KeyCode::Char('z') => app.selected_backends[1] = !app.selected_backends[1],
        KeyCode::Char('n') => app.selected_backends[2] = !app.selected_backends[2],
        KeyCode::Char('a') => {
            let all_selected = app.selected_backends.iter().all(|&b| b);
            app.selected_backends = [!all_selected; 3];
        }
        KeyCode::Enter => {
            if !app.selected_backends.iter().any(|&b| b) {
                app.status_message = Some("Select at least one backend".to_string());
                return;
            }
            app.name_input_buf = format!("~/{}", app.theme_name);
            app.input_mode = InputMode::NameInput;
        }
        KeyCode::Esc => app.input_mode = InputMode::Normal,
        _ => {}
    }
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
    app.palette = assign_slots_with_accent(
        &app.extracted_colors,
        app.mode,
        app.accent,
        app.accent_variant,
    );
    enforce_contrast(&mut app.palette, DEFAULT_ACCENT_CONTRAST);
    app.dirty = true;
    app.selected_slot = None;
    app.status_message = Some(format!("Switched to {mode:?} mode"));
}

/// Key mapping for accent select overlay: (key, accent, variant).
const ACCENT_OPTIONS: [(char, AccentColor, AccentVariant); 20] = [
    ('a', AccentColor::Blue, AccentVariant::Vibrant),
    ('b', AccentColor::Blue, AccentVariant::Muted),
    ('c', AccentColor::Blue, AccentVariant::Dark),
    ('d', AccentColor::Blue, AccentVariant::Pastel),
    ('e', AccentColor::Green, AccentVariant::Vibrant),
    ('f', AccentColor::Green, AccentVariant::Muted),
    ('g', AccentColor::Green, AccentVariant::Dark),
    ('h', AccentColor::Green, AccentVariant::Pastel),
    ('i', AccentColor::Yellow, AccentVariant::Vibrant),
    ('j', AccentColor::Yellow, AccentVariant::Muted),
    ('k', AccentColor::Yellow, AccentVariant::Dark),
    ('l', AccentColor::Yellow, AccentVariant::Pastel),
    ('n', AccentColor::Red, AccentVariant::Vibrant),
    ('o', AccentColor::Red, AccentVariant::Muted),
    ('p', AccentColor::Red, AccentVariant::Dark),
    ('r', AccentColor::Red, AccentVariant::Pastel),
    ('t', AccentColor::Purple, AccentVariant::Vibrant),
    ('u', AccentColor::Purple, AccentVariant::Muted),
    ('v', AccentColor::Purple, AccentVariant::Dark),
    ('w', AccentColor::Purple, AccentVariant::Pastel),
];

/// Gray accent uses a separate key since variants don't affect it.
const GRAY_ACCENT_KEY: char = 'm';

fn apply_accent(app: &mut TuiApp, accent: Option<AccentColor>, variant: AccentVariant) {
    app.accent = accent;
    app.accent_variant = variant;
    app.palette = assign_slots_with_accent(
        &app.extracted_colors,
        app.mode,
        app.accent,
        app.accent_variant,
    );
    enforce_contrast(&mut app.palette, DEFAULT_ACCENT_CONTRAST);
    app.dirty = true;
    app.selected_slot = None;
}

fn handle_accent_select(app: &mut TuiApp, code: KeyCode) {
    match code {
        KeyCode::Esc | KeyCode::Char('0') => {
            apply_accent(app, None, AccentVariant::Vibrant);
            app.status_message = Some("Accent: Off".to_string());
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) if c == GRAY_ACCENT_KEY => {
            apply_accent(app, Some(AccentColor::Gray), AccentVariant::Vibrant);
            app.status_message = Some("Accent: Gray".to_string());
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Char(c) => {
            if let Some(&(_, accent, variant)) = ACCENT_OPTIONS.iter().find(|(k, _, _)| *k == c) {
                apply_accent(app, Some(accent), variant);
                app.status_message = Some(format!("Accent: {:?} {}", accent, variant.label()));
                app.input_mode = InputMode::Normal;
            }
        }
        _ => {}
    }
}

fn regenerate(app: &mut TuiApp) {
    app.seed = app.seed.wrapping_add(1);
    app.extracted_colors = extract_colors_with_seed(&app.pixels, app.k, app.seed);
    app.palette = assign_slots_with_accent(
        &app.extracted_colors,
        app.mode,
        app.accent,
        app.accent_variant,
    );
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

/// Get the effective targets for saving.
fn save_targets(app: &TuiApp) -> Vec<Target> {
    if !app.cli_targets.is_empty() {
        return app.cli_targets.clone();
    }
    let all_targets = [Target::Ghostty, Target::Zellij, Target::Neovim];
    all_targets
        .iter()
        .zip(app.selected_backends.iter())
        .filter(|(_, &selected)| selected)
        .map(|(&t, _)| t)
        .collect()
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        PathBuf::from(home).join(rest)
    } else if path == "~" {
        let home = std::env::var("HOME").unwrap_or_else(|_| "~".to_string());
        PathBuf::from(home)
    } else {
        PathBuf::from(path)
    }
}

/// Compute the save path for a backend, appending its extension if needed.
fn save_path_for_backend(base: &Path, ext: &str) -> PathBuf {
    if ext.is_empty() {
        return base.to_path_buf();
    }
    let s = base.as_os_str().to_string_lossy();
    if s.ends_with(ext) {
        base.to_path_buf()
    } else {
        let mut p = base.as_os_str().to_owned();
        p.push(ext);
        PathBuf::from(p)
    }
}

fn try_save(app: &mut TuiApp) -> Result<()> {
    let raw_path = app.name_input_buf.trim().to_string();
    if raw_path.is_empty() {
        app.status_message = Some("Path cannot be empty".to_string());
        app.input_mode = InputMode::Normal;
        return Ok(());
    }

    let base = expand_tilde(&raw_path);
    let targets = save_targets(app);

    // Check for existing files (overwrite confirmation)
    for target in &targets {
        let backend = get_backend(*target);
        let path = save_path_for_backend(&base, backend.extension());
        if path.exists() {
            app.input_mode = InputMode::ConfirmOverwrite;
            return Ok(());
        }
    }

    do_save(app)
}

fn do_save(app: &mut TuiApp) -> Result<()> {
    let raw_path = app.name_input_buf.trim().to_string();
    let base = expand_tilde(&raw_path);
    let theme_name = base
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("theme")
        .to_string();
    let targets = save_targets(app);
    let mut saved = Vec::new();
    let mut errors = Vec::new();

    for target in &targets {
        let backend = get_backend(*target);
        let path = save_path_for_backend(&base, backend.extension());

        if let Some(parent) = path.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                errors.push(format!("{}: {e}", backend.name()));
                continue;
            }
        }

        match backend.write_to(&app.palette, &theme_name, &path) {
            Ok(_) => saved.push(format!("{} -> {}", backend.name(), path.display())),
            Err(e) => errors.push(format!("{}: {e}", backend.name())),
        }
    }

    app.theme_name = theme_name;
    app.dirty = false;
    app.input_mode = InputMode::Normal;

    if errors.is_empty() {
        let msg = saved.join(", ");
        app.status_message = Some(format!("Saved {msg}"));
    } else {
        let err_str = errors.join("; ");
        if saved.is_empty() {
            app.status_message = Some(format!("Error: {err_str}"));
        } else {
            let ok_str = saved.join(", ");
            app.status_message = Some(format!("Saved {ok_str}; errors: {err_str}"));
        }
    }

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
        InputMode::AccentSelect => draw_accent_select_overlay(f, app),
        InputMode::BackendSelect => draw_backend_select_overlay(f, app),
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

    let accent_label = match app.accent {
        Some(c) => format!("{:?} {}", c, app.accent_variant.label()),
        None => "Off".to_string(),
    };
    let mut lines = vec![
        Line::from(""),
        Line::from(format!("  {}", app.image_path.display())),
        Line::from(""),
        Line::from(format!("  Mode: {:?}", app.mode)),
        Line::from(format!("  Accent: {}", accent_label)),
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
        " d/l: Mode | a: Accent | r: Regen | Tab: Cycle | 1-6: Select | Enter: Save | ?: Help | q: Quit"
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
        Line::from("  a             Cycle accent (off → blue → green → yellow)"),
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
        Line::from("  Save theme to:"),
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

fn draw_confirm_overwrite_overlay(f: &mut Frame, path: &str) {
    let area = centered_rect(50, 20, f.area());
    let lines = vec![
        Line::from(""),
        Line::from(format!("  '{path}' already exists.")),
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

/// Build a compact swatch span for one accent variant: "[k] Label ██ ██ ██ ██ ██ ██"
fn accent_swatch_spans(
    key: char,
    accent: AccentColor,
    variant: AccentVariant,
    mode: ThemeMode,
    is_selected: bool,
) -> Vec<Span<'static>> {
    let preview = preview_monochromatic(accent, variant, mode);
    let mut spans = vec![Span::raw(format!("[{key}]{:<7} ", variant.label()))];
    for c in &preview {
        spans.push(Span::styled(
            "██",
            Style::default().fg(Color::Rgb(c.r, c.g, c.b)),
        ));
    }
    if is_selected {
        spans.push(Span::styled(" ◄", Style::default().fg(Color::Yellow)));
    }
    spans
}

fn draw_accent_select_overlay(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(70, 90, f.area());

    let accents: &[(&str, AccentColor)] = &[
        ("Blue", AccentColor::Blue),
        ("Green", AccentColor::Green),
        ("Yellow", AccentColor::Yellow),
        ("Red", AccentColor::Red),
        ("Purple", AccentColor::Purple),
    ];

    let mut lines = vec![Line::from(""), Line::from("  Select an accent palette:")];

    for (color_name, accent) in accents {
        // Color header with first two variants on one line, next two on the next
        let variants: Vec<(char, AccentVariant)> = ACCENT_OPTIONS
            .iter()
            .filter(|(_, a, _)| a == accent)
            .map(|&(k, _, v)| (k, v))
            .collect();

        lines.push(Line::from(Span::styled(
            format!("  {color_name}:"),
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )));

        // Two variants per row
        for pair in variants.chunks(2) {
            let mut spans = vec![Span::raw("   ")];
            for (i, &(key, variant)) in pair.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::raw("  "));
                }
                let selected = app.accent == Some(*accent) && app.accent_variant == variant;
                spans.extend(accent_swatch_spans(
                    key, *accent, variant, app.mode, selected,
                ));
            }
            lines.push(Line::from(spans));
        }
    }

    // Gray option
    lines.push(Line::from(Span::styled(
        "  Gray:",
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD),
    )));
    let mut gray_spans = vec![Span::raw("   ")];
    let gray_selected = app.accent == Some(AccentColor::Gray);
    gray_spans.extend(accent_swatch_spans(
        GRAY_ACCENT_KEY,
        AccentColor::Gray,
        AccentVariant::Vibrant,
        app.mode,
        gray_selected,
    ));
    lines.push(Line::from(gray_spans));

    lines.push(Line::from(""));
    lines.push(Line::from("  [0] No accent (multi-hue)"));
    lines.push(Line::from("  Press a letter to select | Esc: Cancel"));

    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Accent Palette "))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(Clear, area);
    f.render_widget(popup, area);
}

fn draw_backend_select_overlay(f: &mut Frame, app: &TuiApp) {
    let area = centered_rect(50, 30, f.area());
    let labels = ["Ghostty", "Zellij", "Neovim"];
    let keys = ['G', 'Z', 'N'];
    let mut lines = vec![
        Line::from(""),
        Line::from("  Select backends to save:"),
        Line::from(""),
    ];
    for (i, (label, key)) in labels.iter().zip(keys.iter()).enumerate() {
        let marker = if app.selected_backends[i] {
            "[x]"
        } else {
            "[ ]"
        };
        let style = if app.selected_backends[i] {
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(format!("{marker} [{key}] {label}"), style),
        ]));
    }
    lines.push(Line::from(""));
    lines.push(Line::from("  a: Toggle all | Enter: Confirm | Esc: Cancel"));
    let popup = Paragraph::new(lines)
        .block(Block::bordered().title(" Save Target "))
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
