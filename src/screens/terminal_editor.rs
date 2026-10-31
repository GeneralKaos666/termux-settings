//! Edit the individual colours of the active theme.

use std::path::PathBuf;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};

use crate::app::Command;
use crate::library::{active_theme_path, apply_theme};
use crate::model::colors::{ColorField, PALETTE_SIZE, Rgb, TerminalColors};
use crate::paths::Paths;
use crate::persist::backup::make_backup;
use crate::persist::colors_io::{read_colors, write_colors};
use crate::screens::ScreenAction;
use crate::ui::dialog::{Dialog, DialogResult};
use crate::ui::form;
use crate::ui::input::Field;
use crate::ui::text;
use crate::ui::{Ui, pane, stacked};

/// Number of editable fields: background, foreground, cursor and the palette.
pub const FIELD_COUNT: usize = 3 + PALETTE_SIZE;

/// Form rows the editor assumes are visible until the first render measures
/// the pane. Small on purpose: the focused row must scroll into view even
/// when nothing has been drawn yet.
const DEFAULT_VISIBLE_ROWS: usize = 12;

/// Row holding the `basics` section rule.
const BASICS_ROW: usize = 0;

/// Row holding the `ansi palette` section rule.
const ANSI_ROW: usize = BASICS_ROW + 1 + 3;

/// Total rows the form occupies: two rules plus one row per field.
const FORM_ROWS: usize = ANSI_ROW + 1 + PALETTE_SIZE;

/// How many mock-session lines the preview shows.
const PREVIEW_LINES: usize = 7;

/// Rows the preview's content occupies: its heading, the mock session, the
/// swatch grid and the palette strip, with a blank row between each block.
const PREVIEW_CONTENT_ROWS: u16 = 17;

/// Rows the form keeps before the preview starts shrinking: both borders, the
/// `basics` rule and its three fields.
const MIN_FORM_PANE_ROWS: u16 = 8;

/// Rows the preview needs to show its heading and a few mock-session lines; a
/// shorter preview is dropped so the form keeps the rows.
const MIN_PREVIEW_ROWS: u16 = 9;

/// The step a dialog answer resumes, set just before the dialog opens.
#[derive(Clone, Debug)]
pub enum TerminalPending {
    /// Back up, then write to the theme the editor was opened on.
    WriteAfterBackup {
        target: PathBuf,
        colors: TerminalColors,
    },
    /// The Save As prompt is open for these colours.
    NameForSaveAs { colors: TerminalColors },
    /// The overwrite question is open for an existing target.
    Overwrite {
        target: PathBuf,
        colors: TerminalColors,
    },
    /// Back up, then write a new theme file.
    BackupThenWriteAs {
        target: PathBuf,
        colors: TerminalColors,
    },
}

/// Turn a user-typed theme name into a safe file stem.
///
/// Every run of characters outside `[A-Za-z0-9._-]` becomes `-`, following
/// dashes are trimmed, and an empty result becomes `custom` — the Python
/// authority's rule.
pub fn sanitize_name(name: &str) -> String {
    let mut cleaned = String::new();
    let mut in_run = false;
    for character in name.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-') {
            cleaned.push(character);
            in_run = false;
        } else if !in_run {
            cleaned.push('-');
            in_run = true;
        }
    }
    let cleaned = cleaned.trim_end_matches('-');
    if cleaned.is_empty() {
        "custom".to_owned()
    } else {
        cleaned.to_owned()
    }
}

/// Split `area` into the form on top and the preview below.
fn stacked_areas(area: Rect) -> (Rect, Rect) {
    stacked(
        area,
        PREVIEW_CONTENT_ROWS + 2,
        MIN_FORM_PANE_ROWS,
        MIN_PREVIEW_ROWS,
    )
}

/// The terminal colour editor's state.
pub struct TerminalEditor {
    /// One hex field per [`ColorField`], in editor order.
    pub fields: [Field; FIELD_COUNT],
    /// Whether each field currently holds a parseable colour.
    pub valid: [bool; FIELD_COUNT],
    /// Index into [`TerminalEditor::fields`] with the keyboard.
    pub focus: usize,
    /// First form row drawn, so the focused field stays visible.
    pub scroll: usize,
    /// The theme file saves write to, when a theme is active.
    pub target: Option<PathBuf>,
    /// The colours the fields were loaded from.
    pub base: TerminalColors,
    /// The step a dialog answer resumes.
    pending: Option<TerminalPending>,
    visible_rows: usize,
}

impl TerminalEditor {
    /// Load the fields from the active theme, or from the defaults when no
    /// theme is linked.
    pub fn new(paths: &Paths) -> Self {
        let target = active_theme_path(&paths.colors_properties());
        let base = match &target {
            Some(path) => read_colors(path).unwrap_or_default(),
            None => TerminalColors::default(),
        };
        let fields =
            std::array::from_fn(|index| Field::new(&base.field(ColorField::ALL[index]).to_hex()));
        Self {
            valid: [true; FIELD_COUNT],
            fields,
            focus: 0,
            scroll: 0,
            target,
            base,
            pending: None,
            visible_rows: DEFAULT_VISIBLE_ROWS,
        }
    }

    /// The action the editor emits when it is first opened: preview the
    /// colours it loaded.
    pub fn opened(&mut self) -> ScreenAction {
        ScreenAction::applied(self.current_colors(), "")
    }

    /// The key bindings the help bar shows for this screen.
    pub fn help(&self) -> Vec<(&'static str, &'static str)> {
        vec![
            ("↑/↓", "field"),
            ("Tab", "next"),
            ("ctrl+s", "save"),
            ("ctrl+a", "save as"),
            ("ctrl+r", "revert"),
            ("Esc", "back"),
        ]
    }

    /// Handle a key press, returning the action the app should apply.
    pub fn handle_key(&mut self, key: KeyEvent) -> ScreenAction {
        match key.code {
            KeyCode::Esc => return ScreenAction::Pop,
            KeyCode::Tab | KeyCode::Down => {
                self.focus = (self.focus + 1).min(FIELD_COUNT - 1);
                self.revalidate();
                return ScreenAction::None;
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = self.focus.saturating_sub(1);
                self.revalidate();
                return ScreenAction::None;
            }
            _ => {}
        }
        if self.fields[self.focus].handle_key(key) {
            self.revalidate();
            return ScreenAction::applied(self.current_colors(), "");
        }
        ScreenAction::None
    }

    /// Recompute [`TerminalEditor::valid`] and scroll the focused row into view.
    pub fn revalidate(&mut self) {
        for (field, valid) in self.fields.iter().zip(self.valid.iter_mut()) {
            *valid = Rgb::parse(field.value()).is_some();
        }
        self.scroll_to_focus();
    }

    /// The colours the fields describe, ignoring invalid values.
    pub fn current_colors(&self) -> TerminalColors {
        let mut colors = self.base.clone();
        for (index, field) in self.fields.iter().enumerate() {
            if let Some(color) = Rgb::parse(field.value()) {
                colors.set_field(ColorField::ALL[index], color);
            }
        }
        colors
    }

    /// The colours to save, or the keys that hold invalid values.
    pub fn collect(&self) -> Result<TerminalColors, Vec<String>> {
        let invalid: Vec<String> = self
            .fields
            .iter()
            .enumerate()
            .filter(|(_, field)| Rgb::parse(field.value()).is_none())
            .map(|(index, _)| ColorField::ALL[index].key())
            .collect();
        if invalid.is_empty() {
            Ok(self.current_colors())
        } else {
            Err(invalid)
        }
    }

    /// Run the save or revert flow `command` asks for.
    pub fn command(&mut self, command: Command, paths: &Paths) -> ScreenAction {
        match command {
            Command::Save => self.start_save(paths),
            Command::SaveAs => self.start_save_as(),
            Command::Revert => self.start_revert(),
        }
    }

    /// Save to the active theme, asking for a name or a backup when needed.
    pub fn start_save(&mut self, paths: &Paths) -> ScreenAction {
        let colors = match self.collect() {
            Ok(colors) => colors,
            Err(keys) => return invalid_keys(&keys),
        };
        let Some(target) = self.target.clone() else {
            return self.start_save_as_with(colors);
        };
        let colors_properties = paths.colors_properties();
        if colors_properties.exists() || colors_properties.is_symlink() {
            self.pending = Some(TerminalPending::WriteAfterBackup { target, colors });
            return backup_question();
        }
        self.write_and_apply(&target, colors, paths)
    }

    /// Ask for a name and save the collected colours under it.
    pub fn start_save_as(&mut self) -> ScreenAction {
        match self.collect() {
            Ok(colors) => self.start_save_as_with(colors),
            Err(keys) => invalid_keys(&keys),
        }
    }

    /// Reload the fields from the active file, or from the defaults.
    pub fn start_revert(&mut self) -> ScreenAction {
        self.base = match &self.target {
            Some(path) => read_colors(path).unwrap_or_default(),
            None => TerminalColors::default(),
        };
        for (index, field) in self.fields.iter_mut().enumerate() {
            field.set_value(&self.base.field(ColorField::ALL[index]).to_hex());
        }
        self.revalidate();
        ScreenAction::applied(self.current_colors(), "Reverted to the saved values.")
    }

    /// Advance the save flow once a dialog has answered.
    pub fn on_dialog(&mut self, result: DialogResult, paths: &Paths) -> ScreenAction {
        let Some(pending) = self.pending.take() else {
            return ScreenAction::None;
        };
        match (pending, result) {
            (
                TerminalPending::WriteAfterBackup { target, colors }
                | TerminalPending::BackupThenWriteAs { target, colors },
                _,
            ) => self.write_after_backup(&target, colors, paths),
            (TerminalPending::NameForSaveAs { colors }, DialogResult::Text(Some(name))) => {
                let file = format!("{}.properties", sanitize_name(&name));
                let target = paths.colors_root().join("custom").join(&file);
                if target.exists() {
                    self.pending = Some(TerminalPending::Overwrite { target, colors });
                    ScreenAction::OpenDialog(Dialog::confirm(
                        &format!("{file} already exists. Overwrite?"),
                        "Overwrite",
                        "Cancel",
                    ))
                } else {
                    self.begin_write_as(target, colors, paths)
                }
            }
            (TerminalPending::NameForSaveAs { .. }, _) => {
                ScreenAction::status("Save As cancelled.")
            }
            (TerminalPending::Overwrite { target, colors }, DialogResult::Confirmed(true)) => {
                self.begin_write_as(target, colors, paths)
            }
            (TerminalPending::Overwrite { .. }, _) => ScreenAction::status("Save As cancelled."),
        }
    }

    /// Ask for a name, then save `colors` under it.
    fn start_save_as_with(&mut self, colors: TerminalColors) -> ScreenAction {
        self.pending = Some(TerminalPending::NameForSaveAs { colors });
        ScreenAction::OpenDialog(
            Dialog::prompt("Name for the new theme:", "custom").with_placeholder("my-theme"),
        )
    }

    /// Ask for the backup before writing a new theme file, or write directly.
    fn begin_write_as(
        &mut self,
        target: PathBuf,
        colors: TerminalColors,
        paths: &Paths,
    ) -> ScreenAction {
        let colors_properties = paths.colors_properties();
        if colors_properties.exists() || colors_properties.is_symlink() {
            self.pending = Some(TerminalPending::BackupThenWriteAs { target, colors });
            return backup_question();
        }
        self.write_and_apply(&target, colors, paths)
    }

    /// Back up the current config, then write (cancelling means skip).
    fn write_after_backup(
        &mut self,
        target: &std::path::Path,
        colors: TerminalColors,
        paths: &Paths,
    ) -> ScreenAction {
        if let Err(error) = make_backup(&paths.colors_properties(), &paths.backups_dir(), "colors")
        {
            return ScreenAction::OpenDialog(Dialog::message(
                &format!("Could not back up the current config: {error}"),
                "Backup failed",
            ));
        }
        self.write_and_apply(target, colors, paths)
    }

    /// Write `colors` to `target`, link it when needed, reload and report.
    fn write_and_apply(
        &mut self,
        target: &std::path::Path,
        colors: TerminalColors,
        paths: &Paths,
    ) -> ScreenAction {
        if let Err(error) = write_colors(target, &colors, true) {
            return ScreenAction::status(&format!("could not save {}: {error}", target.display()));
        }
        let colors_properties = paths.colors_properties();
        let same = match (
            std::fs::canonicalize(target),
            std::fs::canonicalize(&colors_properties),
        ) {
            (Ok(target), Ok(active)) => target == active,
            _ => false,
        };
        if !same && let Err(error) = apply_theme(target, &colors_properties) {
            return ScreenAction::status(&format!(
                "could not activate {}: {error}",
                target.display()
            ));
        }
        self.target = std::fs::canonicalize(target).ok();
        self.base = colors;
        let name = target
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| target.display().to_string());
        ScreenAction::status_and_reload(&format!("Saved {name}."))
    }

    /// The form row holding field `index`.
    fn row_of(index: usize) -> usize {
        if index < 3 {
            BASICS_ROW + 1 + index
        } else {
            ANSI_ROW + 1 + (index - 3)
        }
    }

    /// The field index drawn at form row `row`, when the row is a field.
    fn index_at(row: usize) -> usize {
        if row < ANSI_ROW {
            row - BASICS_ROW - 1
        } else {
            row - ANSI_ROW - 1 + 3
        }
    }

    /// Move [`TerminalEditor::scroll`] the least amount that keeps the focused
    /// field inside the visible rows.
    fn scroll_to_focus(&mut self) {
        let row = Self::row_of(self.focus);
        if row < self.scroll {
            self.scroll = row;
        } else if row >= self.scroll + self.visible_rows {
            self.scroll = row + 1 - self.visible_rows;
        }
    }

    /// Draw the form above the preview.
    pub fn render(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        frame.render_widget(
            Block::default().style(
                Style::default()
                    .bg(ui.palette.background)
                    .fg(ui.palette.foreground),
            ),
            area,
        );
        let (form_area, preview_area) = stacked_areas(area);
        self.render_form(frame, form_area, ui);
        self.render_preview(frame, preview_area, ui);
    }

    /// Re-derive the visible row count after a resize.
    pub fn on_resize(&mut self, area: Rect) {
        let (form_area, _) = stacked_areas(area);
        self.visible_rows = form_area.height.saturating_sub(2) as usize;
        self.scroll_to_focus();
    }

    /// Draw the scrolling form of labelled fields.
    fn render_form(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        let block = pane(ui, " terminal colors");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        // Never draw past the pane: `visible_rows` is only updated by
        // `on_resize`, so it can still hold the pre-measure default.
        let visible = self.visible_rows.min(inner.height as usize);
        let end = (self.scroll + visible).min(FORM_ROWS);
        for row in self.scroll..end {
            let row_area = Rect {
                x: inner.x,
                y: inner.y + (row - self.scroll) as u16,
                width: inner.width,
                height: 1,
            };
            if row == BASICS_ROW {
                form::section_rule(frame, row_area, "basics", ui);
            } else if row == ANSI_ROW {
                form::section_rule(frame, row_area, "ansi palette", ui);
            } else {
                let index = Self::index_at(row);
                let label = ColorField::ALL[index].label();
                form::field_row(
                    frame,
                    row_area,
                    &label,
                    &self.fields[index],
                    self.focus == index,
                    ui,
                );
            }
        }
    }

    /// Draw the live preview of the colours the fields currently describe.
    fn render_preview(&self, frame: &mut Frame, area: Rect, ui: &Ui) {
        let block = pane(ui, " preview");
        let inner = block.inner(area);
        frame.render_widget(block, area);
        if inner.width == 0 || inner.height == 0 {
            return;
        }
        let colors = self.current_colors();
        let mut lines = vec![
            Line::from(Span::styled(
                "live preview",
                Style::default()
                    .fg(ui.palette.primary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
        ];
        lines.extend(
            text::mock_session(&colors, ui.glyphs)
                .into_iter()
                .take(PREVIEW_LINES),
        );
        lines.push(Line::from(""));
        lines.extend(text::swatch_grid(&colors));
        lines.push(Line::from(""));
        lines.push(text::palette_strip(&colors));
        frame.render_widget(Paragraph::new(lines), inner);
    }
}

/// The message dialog naming every field that fails validation.
fn invalid_keys(keys: &[String]) -> ScreenAction {
    ScreenAction::OpenDialog(Dialog::message(
        &format!("Invalid hex colour for: {}", keys.join(", ")),
        "Invalid colour",
    ))
}

/// The backup confirmation shared by both save flows.
fn backup_question() -> ScreenAction {
    ScreenAction::OpenDialog(Dialog::confirm(
        "Back up the current config before saving?",
        "Back up",
        "Skip",
    ))
}
