//! The application: screen stack, palette and status line.

use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Frame;
use ratatui::layout::{Constraint, HorizontalAlignment, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Clear, Paragraph};

use crate::chrome;
use crate::library::active_colors;
use crate::model::colors::TerminalColors;
use crate::paths::Paths;
use crate::runtime::{Msg, Runtime, spawn_download, spawn_prompt, spawn_reload, spawn_scan};
use crate::screens::{Screen, ScreenAction, ScreenKind};
use crate::theme::Palette;
use crate::ui::dialog::{Dialog, DialogResult};
use crate::ui::{Glyphs, Ui};

/// Smallest terminal width the screens are drawn at.
pub const MIN_WIDTH: u16 = 40;

/// Smallest terminal height the screens are drawn at.
pub const MIN_HEIGHT: u16 = 10;

/// How long a status message stays on screen.
const STATUS_TTL: Duration = Duration::from_secs(4);

/// A save or revert request, usually mapped from a `Ctrl` chord.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// Save the current editor to its theme.
    Save,
    /// Save the current editor under a new name.
    SaveAs,
    /// Reload the editor from its theme.
    Revert,
}

/// Everything the running application owns.
pub struct App {
    paths: Paths,
    screens: Vec<Screen>,
    dialog: Option<Dialog>,
    colors: TerminalColors,
    palette: Palette,
    glyphs: Glyphs,
    status: Option<(String, Instant)>,
    too_small: bool,
    area: Rect,
    runtime: Runtime,
    quit: bool,
}

impl App {
    /// Build the app on the main menu with Unicode glyphs.
    pub fn new(paths: Paths) -> Self {
        Self::with_glyphs(paths, Glyphs::new(false))
    }

    /// Build the app on the main menu with the given glyph set.
    pub fn with_glyphs(paths: Paths, glyphs: Glyphs) -> Self {
        let colors = active_colors(&paths);
        let main = Screen::new(ScreenKind::Main, &paths);
        Self {
            palette: Palette::from_colors(&colors),
            colors,
            screens: vec![main],
            dialog: None,
            status: None,
            too_small: false,
            area: Rect::default(),
            runtime: Runtime::new(),
            quit: false,
            paths,
            glyphs,
        }
    }

    /// Handle a key press from the event loop.
    pub fn handle_key(&mut self, key: KeyEvent) {
        let quit_chord = key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('q'));
        let quit_plain =
            !key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('q');
        if quit_chord || quit_plain {
            self.request_quit();
            return;
        }
        if self.too_small {
            if key.code == KeyCode::Esc {
                self.request_quit();
            }
            return;
        }
        if let Some(dialog) = self.dialog.as_mut() {
            if let Some(result) = dialog.handle_key(key) {
                self.resolve_dialog(result);
            }
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            let command = match key.code {
                KeyCode::Char('s') => Some(Command::Save),
                KeyCode::Char('a') => Some(Command::SaveAs),
                KeyCode::Char('r') => Some(Command::Revert),
                _ => None,
            };
            if let Some(command) = command {
                self.command(command);
                return;
            }
        }
        let Some(screen) = self.screens.last_mut() else {
            return;
        };
        let action = screen.handle_key(key, &self.paths);
        self.apply(action);
    }

    /// Run a save or revert command against the current screen.
    pub fn command(&mut self, command: Command) {
        let action = match self.screens.last_mut() {
            Some(screen) => screen.command(command, &self.paths),
            None => ScreenAction::None,
        };
        self.apply(action);
    }

    /// The dialog currently on screen, if any.
    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref()
    }

    /// Show `dialog` over the current screen.
    pub fn open_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    /// Close the current dialog and deliver `result` to the screen that asked.
    pub fn resolve_dialog(&mut self, result: DialogResult) {
        self.dialog = None;
        let Some(screen) = self.screens.last_mut() else {
            return;
        };
        let action = screen.on_dialog(result, &self.paths);
        self.apply(action);
    }

    /// Draw the current screen and, when set, the status line.
    ///
    /// The layout is derived from the frame's live area, so the UI reflows as
    /// soon as the terminal shrinks — when the on-screen keyboard opens, for
    /// instance — even if no resize event has been read yet.
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();
        if area != self.area {
            self.set_area(area);
        }
        if self.too_small {
            self.render_too_small(frame, area);
            return;
        }
        let ui = Ui {
            palette: self.palette,
            glyphs: self.glyphs,
        };
        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        self.render_header(frame, header_area, &ui);
        if let Some(screen) = self.screens.last() {
            screen.render(frame, content_area, &ui);
        }
        self.render_footer(frame, footer_area, &ui);
        if let Some(dialog) = &self.dialog {
            frame.render_widget(Clear, area);
            frame.render_widget(dialog.widget(&ui), area);
        }
    }

    /// Draw the one-row header: the gradient title and, on sub-screens, the
    /// breadcrumb section.
    fn render_header(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        let mut title = chrome::gradient_title("termux-settings", ui.palette.dark);
        if self.screens.len() > 1 {
            title.spans.push(Span::styled(
                format!(" {} ", ui.glyphs.breadcrumb()),
                Style::default().fg(ui.palette.muted),
            ));
            title.spans.push(Span::styled(
                self.current().title(),
                Style::default()
                    .fg(chrome::accent(ui.palette.dark))
                    .add_modifier(Modifier::BOLD),
            ));
        }
        frame.render_widget(
            Paragraph::new(title).style(Style::default().bg(ui.palette.panel)),
            area,
        );
    }

    /// Draw the bottom row: the status message when set, otherwise the
    /// context-sensitive help bar.
    fn render_footer(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        if let Some((message, _)) = &self.status {
            frame.render_widget(
                Paragraph::new(message.as_str())
                    .alignment(HorizontalAlignment::Center)
                    .style(
                        Style::default()
                            .bg(ui.palette.panel)
                            .fg(ui.palette.foreground),
                    ),
                area,
            );
        } else {
            let line = chrome::help_bar(
                &self.current().help(),
                chrome::accent(ui.palette.dark),
                ui.palette.muted,
                ui.glyphs,
            );
            frame.render_widget(
                Paragraph::new(line).style(Style::default().bg(ui.palette.panel)),
                area,
            );
        }
    }

    /// Advance time-driven state: expire a stale status message.
    pub fn tick(&mut self, now: Instant) {
        if let Some((_, shown)) = &self.status
            && now.duration_since(*shown) >= STATUS_TTL
        {
            self.status = None;
        }
    }

    /// Apply one message from the event loop or a worker.
    pub fn update(&mut self, msg: Msg) {
        match msg {
            Msg::Tick(now) => self.tick(now),
            Msg::Key(key) => self.handle_key(key),
            Msg::Resize(width, height) => self.set_area(Rect::new(0, 0, width, height)),
            Msg::Reloaded(false) => {
                self.set_status("Run termux-reload-settings to refresh.");
            }
            Msg::Reloaded(true) => {}
            _ => {
                let paths = self.paths.clone();
                if let Some(screen) = self.screens.last_mut() {
                    let action = screen.on_msg(&msg, &paths);
                    self.apply(action);
                }
            }
        }
    }

    /// Record the terminal size and let the current screen re-derive geometry.
    ///
    /// Called for a resize event and, from [`App::render`], whenever the frame
    /// area differs, so a missed event cannot leave stale geometry behind.
    pub fn set_area(&mut self, area: Rect) {
        self.area = area;
        self.too_small = area.width < MIN_WIDTH || area.height < MIN_HEIGHT;
        if self.too_small {
            return;
        }
        let [_header, content, _footer] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);
        if let Some(screen) = self.screens.last_mut() {
            screen.on_resize(content);
        }
    }

    /// Draw the guard shown when the terminal is below [`MIN_WIDTH`] by
    /// [`MIN_HEIGHT`].
    fn render_too_small(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(self.palette.background)
                    .fg(self.palette.foreground),
            ),
            area,
        );
        let row = Rect {
            x: area.x,
            y: area.y + area.height / 2,
            width: area.width,
            height: 1,
        };
        frame.render_widget(
            Paragraph::new(format!(
                "terminal too small ({MIN_WIDTH}×{MIN_HEIGHT} minimum)"
            ))
            .alignment(HorizontalAlignment::Center)
            .style(Style::default().fg(chrome::accent(self.palette.dark))),
            row,
        );
    }

    /// The screen on top of the stack.
    pub fn current(&self) -> &Screen {
        self.screens
            .last()
            .expect("the screen stack is never empty")
    }

    /// The screen on top of the stack, mutably.
    pub fn current_mut(&mut self) -> &mut Screen {
        self.screens
            .last_mut()
            .expect("the screen stack is never empty")
    }

    /// The derived UI palette.
    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Whether the event loop should stop.
    pub fn should_quit(&self) -> bool {
        self.quit
    }

    /// Every worker message that has arrived since the last drain.
    pub fn drain(&self) -> Vec<Msg> {
        self.runtime.drain()
    }

    /// Restore every screen's uncommitted state, then mark the app as quitting.
    pub fn request_quit(&mut self) {
        let paths = self.paths.clone();
        for screen in &mut self.screens {
            screen.on_quit(&paths);
        }
        self.quit = true;
    }

    /// Show `message` in the status line for [`STATUS_TTL`].
    pub fn set_status(&mut self, message: &str) {
        self.status = Some((message.to_owned(), Instant::now()));
    }

    /// Apply the action a screen returned.
    fn apply(&mut self, action: ScreenAction) {
        match action {
            ScreenAction::None => {}
            ScreenAction::Quit => self.request_quit(),
            ScreenAction::Push(kind) => {
                if kind == ScreenKind::FontsTab {
                    spawn_prompt(self.runtime.sender());
                    spawn_scan(self.runtime.sender(), self.paths.clone());
                }
                let mut screen = Screen::new(kind, &self.paths);
                let action = screen.opened(&self.paths);
                self.screens.push(screen);
                self.apply(action);
            }
            ScreenAction::Pop => {
                if self.screens.len() > 1 {
                    self.screens.pop();
                }
                // Reload the revealed screen (e.g. the menu's active-theme
                // name after a change was applied elsewhere).
                if let Some(screen) = self.screens.last_mut() {
                    screen.reload(&self.paths);
                }
            }
            ScreenAction::Applied {
                colors,
                message,
                reload,
            } => {
                if let Some(colors) = colors {
                    self.colors = colors;
                    self.palette = Palette::from_colors(&self.colors);
                }
                if let Some(message) = message {
                    self.set_status(&message);
                }
                if reload {
                    spawn_reload(self.runtime.sender());
                }
            }
            ScreenAction::OpenDialog(dialog) => self.open_dialog(dialog),
            ScreenAction::SpawnDownload { name, url } => {
                spawn_download(self.runtime.sender(), self.paths.clone(), name, url);
            }
        }
    }
}
