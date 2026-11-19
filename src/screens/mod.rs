//! Screen stack entry points.

pub mod app_theme_editor;
pub mod font_catalog;
pub mod fonts_tab;
pub mod main_menu;
pub mod preset_picker;
pub mod terminal_editor;

use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::app::Command;
use crate::model::colors::TerminalColors;
use crate::paths::Paths;
use crate::runtime::Msg;
use crate::ui::Ui;
use crate::ui::dialog::{Dialog, DialogResult};

use self::app_theme_editor::AppThemeEditor;
use self::font_catalog::FontCatalog;
use self::fonts_tab::FontsTab;
use self::main_menu::MainMenu;
use self::preset_picker::PresetPicker;
use self::terminal_editor::TerminalEditor;

/// A screen that can be pushed onto the stack.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ScreenKind {
    Main,
    Presets,
    Terminal,
    AppTheme,
    FontsTab,
    FontCatalog,
}

/// What a screen asks [`crate::app::App`] to do after a key press.
#[derive(Clone, Debug)]
pub enum ScreenAction {
    /// Nothing to do.
    None,
    /// Leave the application.
    Quit,
    /// Open a new screen on top of the current one.
    Push(ScreenKind),
    /// Close the current screen.
    Pop,
    /// Show a modal dialog over the current screen.
    OpenDialog(Dialog),
    /// Re-derive the palette from `colors`, show `message`, and/or ask the app
    /// to reload Termux settings on a worker.
    Applied {
        colors: Option<TerminalColors>,
        message: Option<String>,
        reload: bool,
    },
    /// Download and install a Nerd Font.
    SpawnDownload { name: String, url: String },
}

impl ScreenAction {
    /// Show `msg` in the status line.
    pub fn status(msg: &str) -> Self {
        Self::Applied {
            colors: None,
            message: Some(msg.to_owned()),
            reload: false,
        }
    }

    /// Show `msg` and trigger an asynchronous `termux-reload-settings`.
    pub fn status_and_reload(msg: &str) -> Self {
        Self::Applied {
            colors: None,
            message: Some(msg.to_owned()),
            reload: true,
        }
    }

    /// Re-derive the palette from `colors` and, when `msg` is non-empty, show
    /// it in the status line.
    pub fn applied(colors: TerminalColors, msg: &str) -> Self {
        Self::Applied {
            colors: Some(colors),
            message: (!msg.is_empty()).then(|| msg.to_owned()),
            reload: false,
        }
    }
}

/// The screens this application can show.
pub enum Screen {
    /// The landing menu.
    Main(MainMenu),
    /// The theme library browser.
    Presets(PresetPicker),
    /// The terminal colour editor.
    Terminal(Box<TerminalEditor>),
    /// The `termux.properties` editor.
    AppTheme(Box<AppThemeEditor>),
    FontsTab(FontsTab),
    /// The Nerd Font download screen.
    FontCatalog(FontCatalog),
}

impl Screen {
    /// Build the screen of `kind`.
    pub fn new(kind: ScreenKind, paths: &Paths) -> Self {
        match kind {
            ScreenKind::Main => Self::Main(MainMenu::new(paths)),
            ScreenKind::Presets => Self::Presets(PresetPicker::new(paths)),
            ScreenKind::Terminal => Self::Terminal(Box::new(TerminalEditor::new(paths))),
            ScreenKind::AppTheme => Self::AppTheme(Box::new(AppThemeEditor::new(paths))),
            ScreenKind::FontsTab => Self::FontsTab(FontsTab::new(paths)),
            ScreenKind::FontCatalog => Self::FontCatalog(FontCatalog::new(paths)),
        }
    }

    /// Handle a key press, returning the action the app should apply.
    pub fn handle_key(&mut self, key: KeyEvent, paths: &Paths) -> ScreenAction {
        match self {
            Self::Main(screen) => screen.handle_key(key, paths),
            Self::Presets(screen) => screen.handle_key(key, paths),
            Self::Terminal(screen) => screen.handle_key(key),
            Self::AppTheme(screen) => screen.handle_key(key),
            Self::FontsTab(screen) => screen.handle_key(key, paths),
            Self::FontCatalog(screen) => screen.handle_key(key, paths),
        }
    }

    /// Re-read externally mutable data (for example the active theme) after
    /// the screen is revealed again.
    pub fn reload(&mut self, paths: &Paths) {
        match self {
            Self::Main(screen) => screen.reload(paths),
            Self::Presets(screen) => screen.reload(paths),
            Self::FontsTab(screen) => screen.refresh_entries(paths),
            _ => {}
        }
    }

    /// The section name shown in the breadcrumb header, or `""` at the menu.
    pub fn title(&self) -> &'static str {
        match self {
            Self::Main(_) => "",
            Self::Presets(_) => "Preset Themes",
            Self::Terminal(_) => "Terminal Colors",
            Self::AppTheme(_) => "App Theme",
            Self::FontsTab(_) => "Fonts",
            Self::FontCatalog(_) => "Fonts",
        }
    }

    /// The key bindings the help bar shows for the current screen.
    pub fn help(&self) -> Vec<(&'static str, &'static str)> {
        match self {
            Self::Main(screen) => screen.help(),
            Self::Presets(screen) => screen.help(),
            Self::Terminal(screen) => screen.help(),
            Self::AppTheme(screen) => screen.help(),
            Self::FontsTab(screen) => screen.help(),
            Self::FontCatalog(screen) => screen.help(),
        }
    }

    /// The action a freshly pushed screen wants the app to apply.
    pub fn opened(&mut self, paths: &Paths) -> ScreenAction {
        match self {
            Self::Main(_) => ScreenAction::None,
            Self::Presets(screen) => screen.opened(paths),
            Self::Terminal(screen) => screen.opened(),
            Self::AppTheme(screen) => screen.opened(paths),
            Self::FontsTab(screen) => screen.opened(paths),
            Self::FontCatalog(screen) => screen.opened(paths),
        }
    }

    /// Run a save or revert command from the keyboard.
    pub fn command(&mut self, command: Command, paths: &Paths) -> ScreenAction {
        match self {
            Self::Terminal(screen) => screen.command(command, paths),
            Self::AppTheme(screen) => screen.command(command, paths),
            Self::Main(_) | Self::Presets(_) => ScreenAction::None,
            Self::FontsTab(screen) => screen.command(command, paths),
            Self::FontCatalog(screen) => screen.command(command, paths),
        }
    }

    /// Handle the result of a dialog this screen opened.
    pub fn on_dialog(&mut self, result: DialogResult, paths: &Paths) -> ScreenAction {
        match self {
            Self::Main(screen) => screen.on_dialog(result, paths),
            Self::Presets(screen) => screen.on_dialog(result, paths),
            Self::Terminal(screen) => screen.on_dialog(result, paths),
            Self::AppTheme(screen) => screen.on_dialog(result, paths),
            Self::FontsTab(screen) => screen.on_dialog(result, paths),
            Self::FontCatalog(screen) => screen.on_dialog(result, paths),
        }
    }

    /// Restore any uncommitted state before the application quits.
    pub fn on_quit(&mut self, paths: &Paths) -> ScreenAction {
        match self {
            Self::FontsTab(screen) => screen.on_quit(paths),
            _ => ScreenAction::None,
        }
    }

    /// Re-derive geometry after a resize.
    pub fn on_resize(&mut self, area: Rect) {
        match self {
            Self::Terminal(screen) => screen.on_resize(area),
            Self::AppTheme(screen) => screen.on_resize(area),
            _ => {}
        }
    }

    /// Handle a worker message, if this screen cares.
    pub fn on_msg(&mut self, msg: &Msg, paths: &Paths) -> ScreenAction {
        match self {
            Self::FontsTab(screen) => screen.on_msg(msg, paths),
            Self::FontCatalog(screen) => screen.on_msg(msg, paths),
            _ => ScreenAction::None,
        }
    }

    /// Draw the screen into `area`.
    pub fn render(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        match self {
            Self::Main(screen) => screen.render(frame, area, ui),
            Self::Presets(screen) => screen.render(frame, area, ui),
            Self::Terminal(screen) => screen.render(frame, area, ui),
            Self::AppTheme(screen) => screen.render(frame, area, ui),
            Self::FontsTab(screen) => screen.render(frame, area, ui),
            Self::FontCatalog(screen) => screen.render(frame, area, ui),
        }
    }
}
