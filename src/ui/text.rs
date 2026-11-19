//! Pure text building: the palette strip, swatch grid and the mock shell
//! session.
//!
//! Every function returns owned [`Line`]/[`Span`] values with explicit
//! [`Color::Rgb`] styles, ported from the Python authority's `widgets.py`.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::model::colors::{Rgb, TerminalColors};
use crate::theme::{ensure_contrast, relative_luminance};
use crate::ui::glyphs::Glyphs;

/// Return black or white, whichever reads better on `color`.
///
/// Ports the Python `contrast_color` rule: black above a 0.45 luminance
/// threshold, white otherwise.
pub fn contrast_color(color: Rgb) -> Rgb {
    if relative_luminance(color) > 0.45 {
        Rgb::new(0, 0, 0)
    } else {
        Rgb::new(255, 255, 255)
    }
}

/// A small inline colour chip, `width` cells wide.
pub fn swatch(color: Rgb, width: usize) -> Span<'static> {
    Span::styled(" ".repeat(width), Style::default().bg(rgb(color)))
}

/// A single-line strip of the 16 palette colours, three cells each.
pub fn palette_strip(colors: &TerminalColors) -> Line<'static> {
    Line::from(
        colors
            .palette
            .iter()
            .map(|color| Span::styled("   ", Style::default().bg(rgb(*color))))
            .collect::<Vec<_>>(),
    )
}

/// The 16-chip swatch grid plus background, foreground and cursor samples.
pub fn swatch_grid(colors: &TerminalColors) -> Vec<Line<'static>> {
    let mut lines = Vec::with_capacity(5);
    for start in [0, 8] {
        let mut spans = Vec::new();
        for (index, &background) in colors.palette.iter().enumerate().skip(start).take(8) {
            let foreground = contrast_color(background);
            spans.push(Span::styled(
                format!("{index:>2}"),
                Style::default()
                    .fg(rgb(foreground))
                    .bg(rgb(background))
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(" "));
        }
        lines.push(Line::from(spans));
    }

    lines.push(Line::raw(""));

    let background = colors.background;
    lines.push(Line::from(vec![
        Span::styled(
            "  background  ",
            Style::default()
                .fg(rgb(contrast_color(background)))
                .bg(rgb(background)),
        ),
        Span::raw("  "),
        Span::styled(
            "  foreground  ",
            Style::default()
                .fg(rgb(colors.foreground))
                .bg(rgb(background)),
        ),
    ]));

    let cursor = colors.cursor;
    lines.push(Line::from(Span::styled(
        "  cursor      ",
        Style::default()
            .fg(rgb(contrast_color(cursor)))
            .bg(rgb(cursor)),
    )));

    lines
}

/// A fake shell session rendered in `colors`, one [`Line`] per screen row.
pub fn mock_session(colors: &TerminalColors, glyphs: Glyphs) -> Vec<Line<'static>> {
    let palette = colors.palette;
    let background = colors.background;
    let ink = |index: usize| ensure_contrast(palette[index], background, 2.5);

    let prompt = Line::from(vec![
        bold("user@termux", ink(2)),
        plain(":", colors.foreground),
        bold("~", ink(4)),
        plain("$ ", colors.foreground),
        plain("ls", colors.foreground),
    ]);

    let listing = Line::from(vec![
        Span::raw("  "),
        bold("Documents  ", ink(4)),
        bold("Downloads  ", ink(4)),
        bold("projects  ", ink(4)),
        plain("notes.md  ", colors.foreground),
        plain("README  ", colors.foreground),
    ]);

    let git = Line::from(vec![
        bold("user@termux", ink(2)),
        plain(":", colors.foreground),
        bold("~/projects", ink(4)),
        plain("$ ", colors.foreground),
        plain("git status", colors.foreground),
    ]);

    let branch = Line::from(vec![
        plain("  On branch ", colors.foreground),
        bold("main", ink(6)),
    ]);

    let clean = Line::from(plain("  nothing to commit, working tree clean", ink(2)));

    let tests = Line::from(vec![
        Span::raw("  "),
        bold(glyphs.tick(), ink(2)),
        plain("31 passed", ink(2)),
        plain(" in 2.5s", colors.foreground),
    ]);

    let error = Line::from(vec![
        Span::raw("  "),
        bold("make: ", ink(1)),
        plain("*** [Makefile:12: build] Error 1", ink(1)),
    ]);

    vec![prompt, listing, git, branch, clean, tests, error]
}

/// A foreground-coloured span, optionally bold.
fn plain(content: &'static str, color: Rgb) -> Span<'static> {
    Span::styled(content, Style::default().fg(rgb(color)))
}

/// A bold foreground-coloured span.
fn bold(content: &'static str, color: Rgb) -> Span<'static> {
    Span::styled(
        content,
        Style::default().fg(rgb(color)).add_modifier(Modifier::BOLD),
    )
}

/// Wrap an [`Rgb`] as a [`Color::Rgb`].
fn rgb(color: Rgb) -> Color {
    Color::Rgb(color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_text(lines: &[Line<'static>]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn strip_has_sixteen_coloured_blocks() {
        let line = palette_strip(&TerminalColors::default());
        assert_eq!(line.spans.len(), 16);
        assert_eq!(line.spans[0].style.bg, Some(Color::Rgb(0, 0, 0)));
    }

    #[test]
    fn grid_has_two_chip_rows_and_three_samples() {
        let lines = swatch_grid(&TerminalColors::default());
        assert_eq!(lines.len(), 5);
        assert!(all_text(&lines).contains("15"));
        assert!(all_text(&lines).contains("background"));
    }

    #[test]
    fn mock_session_has_seven_lines_and_a_prompt() {
        let lines = mock_session(
            &TerminalColors::default(),
            crate::ui::glyphs::Glyphs::new(false),
        );
        assert_eq!(lines.len(), 7);
        assert!(all_text(&lines).starts_with("user@termux"));
    }

    #[test]
    fn contrast_color_flips_at_the_threshold() {
        assert_eq!(contrast_color(Rgb::new(255, 255, 255)), Rgb::new(0, 0, 0));
        assert_eq!(contrast_color(Rgb::new(0, 0, 0)), Rgb::new(255, 255, 255));
    }
}
