//! Shared UI context: the derived palette and the active glyph set.

pub mod dialog;
pub mod focus;
pub mod form;
pub mod glyphs;
pub mod input;
pub mod text;

use ratatui::layout::{Constraint, HorizontalAlignment, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders};

use crate::chrome;
use crate::theme::Palette;

pub use glyphs::Glyphs;

/// The rows a pane needs before it can show anything: one for each border.
const MIN_PANE_ROWS: u16 = 2;

/// Split `area` into a primary pane on top and a preview pane below it.
///
/// The preview is given `preview_rows` — its content plus both borders — and
/// the primary pane takes the rest, never dropping below `min_primary_rows`.
/// When the area is too short for both, the preview shrinks first and is
/// dropped once less than `min_preview_rows` is left for it: a shorter preview
/// would only show its borders while the primary pane needs the rows.
pub fn stacked(
    area: Rect,
    preview_rows: u16,
    min_primary_rows: u16,
    min_preview_rows: u16,
) -> (Rect, Rect) {
    let room = area.height.saturating_sub(min_primary_rows + 1);
    let preview = preview_rows.min(room);
    let preview = if preview < min_preview_rows.max(MIN_PANE_ROWS) {
        0
    } else {
        preview
    };
    let [primary, preview] = Layout::vertical([Constraint::Min(0), Constraint::Length(preview)])
        .spacing(u16::from(preview > 0))
        .areas(area);
    (primary, preview)
}

/// Everything a screen needs to draw: the palette and the glyph set.
#[derive(Copy, Clone)]
pub struct Ui {
    pub palette: Palette,
    pub glyphs: Glyphs,
}

/// A bordered pane with the shared accent border, surface fill, and the
/// glyph set's border (ASCII `+ - |` or rounded box-drawing).
pub fn pane<'a>(ui: &Ui, title: &'a str) -> Block<'a> {
    Block::default()
        .borders(Borders::ALL)
        .border_set(ui.glyphs.border_set())
        .title(title)
        .title_alignment(HorizontalAlignment::Left)
        .border_style(Style::default().fg(chrome::accent(ui.palette.dark)))
        .style(Style::default().bg(ui.palette.surface))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(height: u16) -> Rect {
        Rect::new(0, 0, 40, height)
    }

    #[test]
    fn the_preview_keeps_its_height_when_there_is_room() {
        let (primary, preview) = stacked(area(40), 19, 8, 7);
        assert_eq!(preview.height, 19);
        assert_eq!(primary.height, 40 - 1 - 19);
        assert_eq!(preview.y, primary.height + 1);
        assert_eq!(preview.width, 40);
    }

    #[test]
    fn the_primary_pane_keeps_its_minimum_when_space_runs_out() {
        let (primary, preview) = stacked(area(22), 19, 8, 7);
        assert_eq!(primary.height, 8);
        assert_eq!(preview.height, 22 - 1 - 8);
    }

    #[test]
    fn the_preview_is_dropped_when_a_border_cannot_fit() {
        for height in 0..=10 {
            let (primary, preview) = stacked(area(height), 19, 8, 7);
            assert_eq!(preview.height, 0, "height {height}");
            assert_eq!(primary.height, height, "height {height}");
        }
    }

    #[test]
    fn a_preview_too_short_to_show_content_is_dropped() {
        let (primary, preview) = stacked(area(13), 19, 8, 7);
        assert_eq!(preview.height, 0);
        assert_eq!(primary.height, 13);
    }
}
