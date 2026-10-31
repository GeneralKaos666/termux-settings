use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, ListState, Paragraph},
};

use crate::app::Command;
use crate::chrome;
use crate::fonts::{apply, importer, scan};
use crate::model::font_slot::FontSlot;
use crate::paths::Paths;
use crate::runtime::Msg;
use crate::screens::{ScreenAction, ScreenKind};
use crate::ui::dialog::{Dialog, DialogResult};
use crate::ui::input::Field;
use crate::ui::{Ui, pane};

/// Rows the prompt preview pane occupies: its border and one captured line.
const PREVIEW_ROWS: u16 = 3;

/// Rows the slot line occupies.
const SLOT_ROWS: u16 = 2;

/// Rows the font list keeps before the preview, then the slot line, are
/// dropped so the list keeps showing fonts on a short screen.
const MIN_LIST_ROWS: u16 = 10;

#[derive(Clone, Debug)]
pub struct FontsTab {
    entries: Vec<scan::FontEntry>,
    visible: Vec<usize>,
    state: ListState,
    filter: Field,
    show_filter: bool,
    slot: usize,
    session: apply::SessionState,
    prompt: Vec<String>,
    import: Option<PathBuf>,
    load_error: Option<String>,
}

impl FontsTab {
    pub fn new(paths: &Paths) -> Self {
        let (entries, load_error) = scan_entries(paths);
        let visible: Vec<usize> = (0..entries.len()).collect();
        let mut state = ListState::default();
        if !visible.is_empty() {
            state.select(Some(0));
        }
        Self {
            entries,
            visible,
            state,
            filter: Field::new(""),
            show_filter: false,
            slot: 0,
            session: apply::new_session_state(paths),
            prompt: vec!["$ ".to_owned()],
            import: None,
            load_error,
        }
    }

    pub fn refresh_entries(&mut self, paths: &Paths) {
        let (entries, load_error) = scan_entries(paths);
        self.entries = entries;
        self.load_error = load_error;
        self.visible = (0..self.entries.len()).collect();
        self.state.select((!self.visible.is_empty()).then_some(0));
    }

    fn apply_filter(&mut self, needle: &str) {
        let needle = needle.to_lowercase();
        let previous = self
            .state
            .selected()
            .and_then(|row| self.visible.get(row))
            .map(|&index| self.entries[index].name.clone());
        self.visible = (0..self.entries.len())
            .filter(|&index| {
                let entry = &self.entries[index];
                entry.name.to_lowercase().contains(&needle)
                    || entry.family.to_lowercase().contains(&needle)
                    || entry.style.to_lowercase().contains(&needle)
            })
            .collect();
        let keep = previous.and_then(|name| {
            self.visible
                .iter()
                .position(|&index| self.entries[index].name == name)
        });
        self.state
            .select(keep.or_else(|| (!self.visible.is_empty()).then_some(0)));
    }

    pub fn handle_key(&mut self, key: KeyEvent, paths: &Paths) -> ScreenAction {
        if self.show_filter {
            match key.code {
                KeyCode::Esc => {
                    self.show_filter = false;
                    self.filter.set_value("");
                    self.apply_filter("");
                    return ScreenAction::None;
                }
                KeyCode::Enter => {
                    self.show_filter = false;
                    return ScreenAction::None;
                }
                _ => {
                    if self.filter.handle_key(key) {
                        let needle = self.filter.value().to_owned();
                        self.apply_filter(&needle);
                    }
                    return ScreenAction::None;
                }
            }
        }
        match key.code {
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(i) = self.state.selected()
                    && i + 1 < self.visible.len()
                {
                    self.state.select(Some(i + 1));
                }
                ScreenAction::None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(i) = self.state.selected() {
                    self.state.select(Some(i.saturating_sub(1)));
                }
                ScreenAction::None
            }
            KeyCode::Char('s') => {
                self.slot = (self.slot + 1) % FontSlot::ALL.len();
                ScreenAction::status(&format!("Slot: {}", FontSlot::ALL[self.slot].as_str()))
            }
            KeyCode::Char('p') | KeyCode::Char(' ') => {
                let selected = self
                    .selected_entry()
                    .map(|entry| (entry.path.clone(), entry.name.clone()));
                if let Some((path, name)) = selected {
                    match apply::preview_font(
                        paths,
                        &path,
                        FontSlot::ALL[self.slot],
                        &mut self.session,
                    ) {
                        Ok(_) => ScreenAction::status_and_reload(&format!("Previewing {name}")),
                        Err(error) => ScreenAction::status(&format!("Preview failed: {error}")),
                    }
                } else {
                    ScreenAction::None
                }
            }
            KeyCode::Enter => {
                if apply::is_dirty(&self.session) {
                    let _ = apply::commit_preview(paths, &mut self.session);
                    return ScreenAction::status("Font applied");
                }
                ScreenAction::None
            }
            KeyCode::Esc => {
                if apply::is_dirty(&self.session) {
                    let _ = apply::restore_original(paths, &mut self.session);
                }
                ScreenAction::Pop
            }
            KeyCode::Char('i') => ScreenAction::OpenDialog(
                Dialog::prompt("Font path to import:", "").with_placeholder("~/font.ttf"),
            ),
            KeyCode::Char('d') => ScreenAction::Push(ScreenKind::FontCatalog),
            KeyCode::Char('/') => {
                self.show_filter = true;
                ScreenAction::None
            }
            _ => ScreenAction::None,
        }
    }

    fn selected_entry(&self) -> Option<&scan::FontEntry> {
        let row = self.state.selected()?;
        let index = *self.visible.get(row)?;
        self.entries.get(index)
    }

    pub fn opened(&mut self, paths: &Paths) -> ScreenAction {
        self.refresh_entries(paths);
        if let Some(error) = &self.load_error {
            return ScreenAction::status(&format!("Font library unreadable: {error}"));
        }
        ScreenAction::None
    }
    pub fn command(&mut self, _command: Command, _paths: &Paths) -> ScreenAction {
        ScreenAction::None
    }

    pub fn on_dialog(&mut self, result: DialogResult, paths: &Paths) -> ScreenAction {
        if let Some(src) = self.import.take() {
            if let DialogResult::Confirmed(keep_both) = result {
                let mode = if keep_both { "keep-both" } else { "replace" };
                return self.finish_import(paths, &src, mode);
            }
            return ScreenAction::None;
        }
        if let DialogResult::Text(Some(path)) = result {
            let src = PathBuf::from(path);
            let dest = paths.fonts_dir().join(src.file_name().unwrap_or_default());
            if dest.exists() {
                self.import = Some(src);
                return ScreenAction::OpenDialog(Dialog::confirm(
                    &format!(
                        "Font already in library: {}. Keep both?",
                        dest.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    "Keep both",
                    "Replace",
                ));
            }
            return self.finish_import(paths, &src, "keep-both");
        }
        ScreenAction::None
    }

    fn finish_import(&mut self, paths: &Paths, src: &std::path::Path, mode: &str) -> ScreenAction {
        match importer::import_file(paths, src, mode) {
            Ok(dest) => {
                self.refresh_entries(paths);
                ScreenAction::status(&format!(
                    "Imported {}",
                    dest.file_name().unwrap_or_default().to_string_lossy()
                ))
            }
            Err(error) => ScreenAction::status(&format!("Import failed: {error}")),
        }
    }

    /// Restore any uncommitted preview before the application quits.
    pub fn on_quit(&mut self, paths: &Paths) -> ScreenAction {
        if apply::is_dirty(&self.session) {
            let _ = apply::restore_original(paths, &mut self.session);
        }
        ScreenAction::None
    }

    /// Handle a worker message: a captured prompt or a fresh library scan.
    pub fn on_msg(&mut self, msg: &Msg, _paths: &Paths) -> ScreenAction {
        match msg {
            Msg::PromptCaptured(lines) => {
                self.prompt = lines.clone();
                ScreenAction::None
            }
            Msg::FontsScanned(Ok(entries)) => {
                self.entries = entries.clone();
                self.load_error = None;
                self.visible = (0..self.entries.len()).collect();
                if !self.visible.is_empty() && self.state.selected().is_none() {
                    self.state.select(Some(0));
                }
                ScreenAction::None
            }
            Msg::FontsScanned(Err(error)) => {
                self.entries.clear();
                self.visible.clear();
                self.load_error = Some(error.to_string());
                ScreenAction::status(&format!("Font library unreadable: {error}"))
            }
            _ => ScreenAction::None,
        }
    }

    /// The key bindings the help bar shows for this screen.
    pub fn help(&self) -> Vec<(&'static str, &'static str)> {
        if self.show_filter {
            vec![("type", "filter"), ("Enter", "done"), ("Esc", "clear")]
        } else {
            vec![
                ("↑/↓", "move"),
                ("p", "preview"),
                ("Enter", "apply"),
                ("s", "slot"),
                ("i", "import"),
                ("d", "download"),
                ("/", "filter"),
                ("Esc", "back"),
            ]
        }
    }

    pub fn render(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(ui.palette.background)
                    .fg(ui.palette.foreground),
            ),
            area,
        );
        let preview_rows = if area.height >= MIN_LIST_ROWS + SLOT_ROWS + PREVIEW_ROWS {
            PREVIEW_ROWS
        } else {
            0
        };
        let slot_rows = if area.height >= MIN_LIST_ROWS + SLOT_ROWS {
            SLOT_ROWS
        } else {
            0
        };
        let [preview_area, slot_area, list_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(preview_rows),
                Constraint::Length(slot_rows),
                Constraint::Min(0),
            ])
            .areas(area);
        // preview area
        let preview: Vec<Line> = self.prompt.iter().map(|l| Line::from(l.as_str())).collect();
        frame.render_widget(
            Paragraph::new(preview).block(pane(ui, " Preview")),
            preview_area,
        );
        // slot info
        let (open, close) = ui.glyphs.chip();
        let slot = FontSlot::ALL[self.slot].as_str();
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("slot  ", Style::default().fg(ui.palette.muted)),
                Span::styled(
                    format!("{open} {slot} {close}"),
                    Style::default()
                        .fg(chrome::accent(ui.palette.dark))
                        .add_modifier(Modifier::BOLD),
                ),
            ])),
            slot_area,
        );
        // font list
        let title = if slot_rows == 0 {
            format!(" Fonts ({}) · slot {slot}", self.visible.len())
        } else {
            format!(" Fonts ({})", self.visible.len())
        };
        let block = pane(ui, &title);
        let inner = block.inner(list_area);
        frame.render_widget(block, list_area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let [filter_area, list_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(inner);

        let filter_style = if self.show_filter {
            Style::default()
                .bg(ui.palette.boost)
                .fg(ui.palette.foreground)
        } else {
            Style::default().fg(ui.palette.muted)
        };
        let filter_text = if self.filter.value().is_empty() {
            format!("{}filter fonts", ui.glyphs.filter())
        } else {
            self.filter.value().to_owned()
        };
        frame.render_widget(Paragraph::new(filter_text).style(filter_style), filter_area);

        let items: Vec<ListItem> = if self.visible.is_empty() {
            vec![ListItem::new(
                Line::from("No fonts.").style(Style::default().fg(ui.palette.muted)),
            )]
        } else {
            self.visible
                .iter()
                .filter_map(|&index| self.entries.get(index))
                .map(|e| ListItem::new(format!("{} - {} {}", e.name, e.family, e.style)))
                .collect()
        };
        let list = List::new(items)
            .highlight_symbol(ui.glyphs.marker())
            .highlight_style(
                Style::default()
                    .bg(chrome::accent(ui.palette.dark))
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        let mut list_state = self.state;
        frame.render_stateful_widget(list, list_area, &mut list_state);
    }
}

/// Scan the font library, distinguishing an unreadable directory from an empty
/// one so the screen can tell the user rather than showing a blank list.
fn scan_entries(paths: &Paths) -> (Vec<scan::FontEntry>, Option<String>) {
    match scan::list_library(paths) {
        Ok(entries) => (entries, None),
        Err(error) => (Vec::new(), Some(error.to_string())),
    }
}
