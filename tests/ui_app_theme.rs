mod support;

use crossterm::event::KeyCode;
use std::os::unix::fs::PermissionsExt;
use support::{
    app_theme_mut, app_theme_with, focus_row, key, open_app_theme, render_text, row_index, set_row,
};
use termux_settings::app::{App, Command};
use termux_settings::paths::Paths;
use termux_settings::ui::dialog::DialogResult;

#[test]
fn rows_initialise_from_the_file_in_schema_order() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::write(
        paths.termux_properties(),
        "terminal-cursor-style = bar\nuse-black-ui = true\n",
    )
    .unwrap();
    let mut app = App::new(paths);
    open_app_theme(&mut app);
    let text = render_text(&mut app, 100, 34);
    assert!(text.contains("Cursor style"));
    assert!(text.contains("bar"));
    assert!(text.contains("Black UI"));
}

#[test]
fn the_focused_row_stays_visible_when_the_view_shrinks() {
    let (mut app, _paths) = app_theme_with("");
    let last = app_theme_mut(&mut app).rows.len() - 1;
    focus_row(&mut app, last);

    let tall = render_text(&mut app, 100, 40);
    assert!(tall.contains("Allow external apps"));

    let short = render_text(&mut app, 100, 14);
    assert!(
        short.contains("Allow external apps"),
        "the focused row scrolled out of a shorter view:\n{short}"
    );
}

#[test]
fn invalid_number_blocks_the_save_with_a_message() {
    let (mut app, _paths) = app_theme_with("terminal-margin-horizontal = 5\n");
    set_row(&mut app, "terminal-margin-horizontal", "999");
    app.command(Command::Save);
    assert!(render_text(&mut app, 100, 34).contains("Margin horizontal must be <= 50"));
}

#[test]
fn save_backs_up_and_preserves_unmanaged_lines() {
    let (mut app, paths) = app_theme_with("use-black-ui = true\nextra-keys = [['ESC','-']]\n");
    set_row(&mut app, "use-black-ui", "false");
    app.command(Command::Save);
    app.resolve_dialog(DialogResult::Confirmed(true));
    let out = std::fs::read_to_string(paths.termux_properties()).unwrap();
    assert!(out.contains("use-black-ui = false"));
    assert!(out.contains("extra-keys = [['ESC','-']]"));
}

#[test]
fn selectors_cycle_through_choices_and_unset() {
    let (mut app, _paths) = app_theme_with("");
    let bell = row_index(&mut app, "bell-character");
    focus_row(&mut app, bell);
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Right));
    app.handle_key(key(KeyCode::Right));
    let rendered = render_text(&mut app, 100, 34);
    assert!(rendered.contains("ignore"));
}

#[test]
fn unreadable_theme_shows_dialog_and_blocks_save() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::write(paths.termux_properties(), "use-black-ui = true\n").unwrap();
    std::fs::set_permissions(
        paths.termux_properties(),
        std::fs::Permissions::from_mode(0o000),
    )
    .unwrap();

    let mut app = App::new(paths.clone());
    open_app_theme(&mut app);
    assert!(app.dialog().is_some());

    app.command(Command::Save);

    std::fs::set_permissions(
        paths.termux_properties(),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(paths.termux_properties()).unwrap(),
        "use-black-ui = true\n"
    );
}
