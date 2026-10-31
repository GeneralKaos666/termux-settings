//! Browse the theme library and activate a theme.

use std::collections::HashMap;
use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};

use crate::chrome;
use crate::library::{ThemeRef, active_theme_path, apply_theme, scan_themes};
use crate::model::colors::TerminalColors;
use crate::paths::Paths;
use crate::persist::colors_io::read_colors;
use crate::presets::materialize;
use crate::screens::ScreenAction;
use crate::ui::focus::{Move, list_move};
use crate::ui::input::Field;
use crate::ui::text;
use crate::ui::{Ui, pane, stacked};

/// Rows a page key jumps in the theme list.
const PAGE: usize = 10;

/// How many mock-session lines the preview shows.
const PREVIEW_LINES: usize = 5;

/// Rows the preview's content occupies: the theme name, the mock session, the
/// swatch grid, the palette strip and the path, with a blank row between them.
const PREVIEW_CONTENT_ROWS: u16 = 17;

/// Rows the list keeps before the preview starts shrinking: both borders, the
/// filter line and a few themes.
const MIN_LIST_PANE_ROWS: u16 = 8;

/// Rows the preview needs to show its name and a few mock-session lines; a
/// shorter preview is dropped so the list keeps the rows.
const MIN_PREVIEW_ROWS: u16 = 7;

/// Which region of the picker has the keyboard.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Focus {
    List,
    Filter,
}

/// The theme picker's state.
pub struct PresetPicker {
    /// Every theme found in the library.
    pub all: Vec<ThemeRef>,
    /// Indices into [`PresetPicker::all`] that match the filter, in list order.
    pub visible: Vec<usize>,
    /// The filter text.
    pub filter: Field,
    /// The highlighted row.
    pub list: ListState,
    /// Parsed colours per theme path.
    pub cache: HashMap<PathBuf, TerminalColors>,
    /// Canonical path per theme path, memoized for active detection.
    canon: HashMap<PathBuf, PathBuf>,
    /// The theme file `colors.properties` currently resolves to.
    pub active: Option<PathBuf>,
    /// The theme library root, for computing the active theme's label.
    colors_root: PathBuf,
    focus: Focus,
}

/// Scan the library, seeding the built-in themes when it is empty.
pub fn load_themes(paths: &Paths) -> Vec<ThemeRef> {
    let root = paths.colors_root();
    let mut themes = scan_themes(&root);
    if themes.is_empty() {
        let _ = materialize(&root.join("builtin"));
        themes = scan_themes(&root);
    }
    themes
}

impl PresetPicker {
    /// Build the picker for `paths`, selecting the first theme.
    pub fn new(paths: &Paths) -> Self {
        let all = load_themes(paths);
        let visible: Vec<usize> = (0..all.len()).collect();
        let list = if visible.is_empty() {
            ListState::default()
        } else {
            ListState::default().with_selected(Some(0))
        };
        let mut picker = Self {
            active: active_theme_path(&paths.colors_properties()),
            all,
            visible,
            filter: Field::new(""),
            list,
            cache: HashMap::new(),
            canon: HashMap::new(),
            colors_root: paths.colors_root(),
            focus: Focus::List,
        };
        picker.fill_cache();
        picker
    }

    /// The theme under the highlighted row.
    pub fn selected(&self) -> Option<&ThemeRef> {
        let row = self.list.selected()?;
        self.all.get(*self.visible.get(row)?)
    }

    /// The highlighted row, if the list has one.
    fn selected_row(&self) -> Option<usize> {
        self.list.selected().filter(|row| *row < self.visible.len())
    }

    /// Read every theme's colours and canonical path once, up front, so the
    /// draw path never touches the filesystem.
    fn fill_cache(&mut self) {
        for theme in &self.all {
            self.cache
                .entry(theme.path.clone())
                .or_insert_with(|| read_colors(&theme.path).unwrap_or_default());
            self.canon.entry(theme.path.clone()).or_insert_with(|| {
                std::fs::canonicalize(&theme.path).unwrap_or_else(|_| theme.path.clone())
            });
        }
    }

    /// The colours of the theme at `row`, read once and cached.
    ///
    /// Unreadable or malformed themes fall back to the default colours so the
    /// preview keeps working.
    pub fn colors_for(&self, row: usize) -> TerminalColors {
        let Some(theme) = self.visible.get(row).and_then(|&index| self.all.get(index)) else {
            return TerminalColors::default();
        };
        self.cache.get(&theme.path).cloned().unwrap_or_default()
    }

    /// The active theme's library-relative name, or `default`.
    fn active_label(&self) -> String {
        let Some(active) = &self.active else {
            return "default".to_owned();
        };
        active
            .strip_prefix(&self.colors_root)
            .map(|rel| rel.with_extension("").to_string_lossy().into_owned())
            .unwrap_or_else(|_| "default".to_owned())
    }

    /// Keep only the themes whose label contains `needle`, ignoring case.
    ///
    /// The selection follows the theme it was on when that theme survives the
    /// filter; otherwise it moves to the first row.
    pub fn apply_filter(&mut self, needle: &str) {
        let previous = self.selected().map(|theme| theme.path.clone());
        let needle = needle.to_lowercase();
        self.visible = (0..self.all.len())
            .filter(|&index| {
                self.all
                    .get(index)
                    .is_some_and(|theme| theme.label().to_lowercase().contains(&needle))
            })
            .collect();
        let keep = previous.and_then(|path| {
            self.visible
                .iter()
                .position(|&index| self.all.get(index).is_some_and(|theme| theme.path == path))
        });
        self.list
            .select(keep.or_else(|| (!self.visible.is_empty()).then_some(0)));
    }

    /// The action the picker emits when it is first opened: preview the first
    /// theme, as the Python screen does on mount.
    pub fn opened(&mut self, _paths: &Paths) -> ScreenAction {
        match self.selected_row() {
            Some(row) => ScreenAction::applied(self.colors_for(row), ""),
            None => ScreenAction::None,
        }
    }

    /// The key bindings the help bar shows for this screen, by focus.
    pub fn help(&self) -> Vec<(&'static str, &'static str)> {
        match self.focus {
            Focus::List => vec![
                ("↑/↓", "move"),
                ("Enter", "apply"),
                ("r", "reload"),
                ("Tab", "filter"),
            ],
            Focus::Filter => vec![("type", "filter"), ("Enter", "done"), ("Esc", "clear")],
        }
    }

    /// Handle a key press, returning the action the app should apply.
    pub fn handle_key(&mut self, key: KeyEvent, paths: &Paths) -> ScreenAction {
        match key.code {
            KeyCode::Esc => return ScreenAction::Pop,
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::List {
                    Focus::Filter
                } else {
                    Focus::List
                };
                return ScreenAction::None;
            }
            _ => {}
        }
        match self.focus {
            Focus::List => self.handle_list_key(key, paths),
            Focus::Filter => self.handle_filter_key(key),
        }
    }

    /// Handle a key press while the theme list has focus.
    fn handle_list_key(&mut self, key: KeyEvent, paths: &Paths) -> ScreenAction {
        match key.code {
            KeyCode::Enter => return self.apply(paths),
            KeyCode::Char('r') => {
                self.reload(paths);
                return self.refresh();
            }
            _ => {}
        }
        let movement = match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(Move::Up),
            KeyCode::Down | KeyCode::Char('j') => Some(Move::Down),
            KeyCode::Home => Some(Move::Home),
            KeyCode::End => Some(Move::End),
            KeyCode::PageUp => Some(Move::PageUp),
            KeyCode::PageDown => Some(Move::PageDown),
            _ => None,
        };
        if let Some(movement) = movement {
            list_move(&mut self.list, self.visible.len(), movement, PAGE);
            return self.refresh();
        }
        ScreenAction::None
    }

    /// Handle a key press while the filter has focus.
    fn handle_filter_key(&mut self, key: KeyEvent) -> ScreenAction {
        if key.code == KeyCode::Enter {
            self.focus = Focus::List;
            return ScreenAction::None;
        }
        if self.filter.handle_key(key) {
            let needle = self.filter.value().to_owned();
            self.apply_filter(&needle);
            return self.refresh();
        }
        ScreenAction::None
    }

    /// Refresh the whole UI from the highlighted theme, if any.
    fn refresh(&mut self) -> ScreenAction {
        match self.selected_row() {
            Some(row) => ScreenAction::applied(self.colors_for(row), ""),
            None => ScreenAction::None,
        }
    }

    /// Re-scan the library, keeping the filter and, when possible, the
    /// highlighted theme.
    pub fn reload(&mut self, paths: &Paths) {
        let previous = self.selected().map(|theme| theme.path.clone());
        self.all = load_themes(paths);
        self.cache.clear();
        self.canon.clear();
        self.fill_cache();
        let needle = self.filter.value().to_owned();
        self.visible = (0..self.all.len()).collect();
        self.apply_filter(&needle);
        if let Some(path) = previous {
            let row = self
                .visible
                .iter()
                .position(|&index| self.all.get(index).is_some_and(|theme| theme.path == path));
            if let Some(row) = row {
                self.list.select(Some(row));
            }
        }
        self.active = active_theme_path(&paths.colors_properties());
    }

    /// Link `colors.properties` to the highlighted theme.
    pub fn apply(&mut self, paths: &Paths) -> ScreenAction {
        let Some(theme) = self.selected().cloned() else {
            return ScreenAction::None;
        };
        if apply_theme(&theme.path, &paths.colors_properties()).is_err() {
            return ScreenAction::status(&format!("theme not found: {}", theme.path.display()));
        }
        self.active = std::fs::canonicalize(&theme.path).ok();
        ScreenAction::status_and_reload(&format!("Saved {}.", theme.label()))
    }

    /// Whether `theme` is the one currently linked from `colors.properties`.
    fn is_active(&self, theme: &ThemeRef) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| self.canon.get(&theme.path) == Some(active))
    }

    /// Draw the filter, the theme list and the preview.
    pub fn render(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(ui.palette.background)
                    .fg(ui.palette.foreground),
            ),
            area,
        );
        let (list_area, preview_area) = stacked(
            area,
            PREVIEW_CONTENT_ROWS + 2,
            MIN_LIST_PANE_ROWS,
            MIN_PREVIEW_ROWS,
        );
        self.render_list(frame, list_area, ui);
        self.render_preview(frame, preview_area, ui);
    }

    /// Draw the bordered filter-plus-list pane.
    fn render_list(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        let title = format!(
            " themes ({}) · active: {}",
            self.visible.len(),
            self.active_label()
        );
        let block = pane(ui, &title);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let [filter_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);

        let filter_style = if self.focus == Focus::Filter {
            Style::default()
                .bg(ui.palette.boost)
                .fg(ui.palette.foreground)
        } else {
            Style::default().fg(ui.palette.muted)
        };
        let text = if self.filter.value().is_empty() {
            format!("{}filter themes", ui.glyphs.filter())
        } else {
            self.filter.value().to_owned()
        };
        frame.render_widget(Paragraph::new(text).style(filter_style), filter_area);

        let items: Vec<ListItem> = if self.visible.is_empty() {
            vec![ListItem::new(
                Line::from("No themes found.").style(Style::default().fg(ui.palette.muted)),
            )]
        } else {
            self.visible
                .iter()
                .filter_map(|&index| self.all.get(index))
                .map(|theme| {
                    let mut spans = vec![Span::raw(theme.label())];
                    if self.is_active(theme) {
                        spans.push(Span::styled(
                            format!("  {} active", ui.glyphs.dot()),
                            Style::default()
                                .fg(chrome::accent(ui.palette.dark))
                                .add_modifier(Modifier::BOLD),
                        ));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect()
        };
        let list = List::new(items)
            .highlight_symbol(ui.glyphs.marker())
            .highlight_style(
                Style::default()
                    .bg(chrome::accent(ui.palette.dark))
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            )
            .style(Style::default().bg(ui.palette.surface));

        let list_rows = list_area.height as usize;
        if self.visible.len() > list_rows {
            let mut scroll = ScrollbarState::new(self.visible.len())
                .position(self.list.selected().unwrap_or(0).min(self.visible.len()));
            frame.render_stateful_widget(
                Scrollbar::default().orientation(ScrollbarOrientation::VerticalRight),
                list_area,
                &mut scroll,
            );
        }

        let mut list_state = self.list;
        frame.render_stateful_widget(list, list_area, &mut list_state);
    }

    /// Draw the preview pane for the highlighted theme.
    fn render_preview(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        let block = pane(ui, " preview");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let Some(row) = self.selected_row() else {
            frame.render_widget(
                Paragraph::new("No themes found.").style(Style::default().fg(ui.palette.muted)),
                inner,
            );
            return;
        };
        let Some(theme) = self.selected().cloned() else {
            return;
        };
        let colors = self.colors_for(row);

        let mut name = vec![Span::styled(
            theme.label(),
            Style::default()
                .fg(ui.palette.primary)
                .add_modifier(Modifier::BOLD),
        )];
        if self.is_active(&theme) {
            name.push(Span::styled(
                format!("  {} active", ui.glyphs.dot()),
                Style::default()
                    .fg(chrome::accent(ui.palette.dark))
                    .add_modifier(Modifier::BOLD),
            ));
        }
        let mut lines = vec![Line::from(name), Line::from("")];
        lines.extend(
            text::mock_session(&colors, ui.glyphs)
                .into_iter()
                .take(PREVIEW_LINES),
        );
        lines.push(Line::from(""));
        lines.extend(text::swatch_grid(&colors));
        lines.push(Line::from(""));
        lines.push(text::palette_strip(&colors));
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            format!("{} · {}", theme.group, theme.path.display()),
            Style::default().fg(ui.palette.muted),
        )));
        frame.render_widget(Paragraph::new(lines), inner);
    }
}
