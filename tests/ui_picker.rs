mod support;

use support::{key, open_picker, picker_theme_path, picker_with, render_text};
use termux_settings::app::App;
use termux_settings::paths::Paths;
use termux_settings::screens::preset_picker::load_themes;
use termux_settings::ui::dialog::DialogResult;
use termux_settings::ui::focus::{Move, list_move};

#[test]
fn header_shows_active_theme_label() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.colors_root().join("dark")).unwrap();
    let theme = paths.colors_root().join("dark/T.properties");
    std::fs::write(&theme, "background=#000000\n").unwrap();
    std::os::unix::fs::symlink(&theme, paths.colors_properties()).unwrap();

    let mut app = App::new(paths);
    open_picker(&mut app);
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("dark/T"));
}

#[test]
fn scrollbar_appears_when_list_overflows() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    for i in 0..40 {
        std::fs::write(
            paths.colors_root().join(format!("theme{i:02}.properties")),
            "background=#000000\n",
        )
        .unwrap();
    }

    let mut app = App::new(paths);
    open_picker(&mut app);
    let rendered = render_text(&mut app, 100, 20);
    assert!(rendered.contains('█'), "expected a scrollbar thumb");
}

#[test]
fn empty_library_is_seeded_with_builtins() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let themes = load_themes(&paths);
    assert_eq!(themes.len(), 6);
    assert!(
        dir.path()
            .join("colors/builtin/dracula.properties")
            .is_file()
    );
}

#[test]
fn filter_is_case_insensitive_and_keeps_a_visible_selection() {
    let mut picker = picker_with(&["Dracula", "Nord", "Solarized"]);
    picker.apply_filter("dra");
    assert_eq!(picker.visible.len(), 1);
    assert_eq!(picker.selected().unwrap().name, "Dracula");
    picker.apply_filter("");
    assert_eq!(picker.visible.len(), 3);
}

#[test]
fn movement_clamps_and_pages() {
    let mut picker = picker_with(&["a", "b", "c", "d"]);
    list_move(&mut picker.list, 4, Move::Down, 2);
    assert_eq!(picker.list.selected(), Some(1));
    for _ in 0..10 {
        list_move(&mut picker.list, 4, Move::Down, 2);
    }
    assert_eq!(picker.list.selected(), Some(3));
    list_move(&mut picker.list, 4, Move::Home, 2);
    assert_eq!(picker.list.selected(), Some(0));
    list_move(&mut picker.list, 4, Move::PageDown, 2);
    assert_eq!(picker.list.selected(), Some(2));
}

#[test]
fn picker_renders_labels_and_the_count() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_picker(&mut app);
    let text = render_text(&mut app, 100, 30);
    assert!(text.contains("themes (6)"));
    assert!(text.contains("dracula"));
}

#[test]
fn escape_returns_to_the_menu() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Esc));
    assert!(
        render_text(&mut app, 100, 30).contains("Browse Presets")
            || render_text(&mut app, 100, 30).contains("Colors")
    );
}

#[test]
fn moving_the_selection_retints_the_ui() {
    use ratatui::style::Color;
    use termux_settings::paths::Paths;

    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_picker(&mut app);
    let before = *app.palette();
    app.handle_key(key(crossterm::event::KeyCode::Down));
    assert_ne!(before, *app.palette());
    assert!(app.palette().background != Color::Rgb(0, 0, 0));
}

#[test]
fn enter_links_colors_properties_to_the_theme() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Enter));
    let link = paths.colors_properties();
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(render_text(&mut app, 100, 30).contains("Saved"));
}

#[test]
fn apply_failure_reports_and_keeps_the_old_link() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Enter));
    let applied = std::fs::canonicalize(paths.colors_properties()).unwrap();

    let theme = picker_theme_path(&mut app);
    std::fs::remove_file(theme).unwrap();
    app.handle_key(key(crossterm::event::KeyCode::Enter));

    assert!(render_text(&mut app, 100, 30).contains("not found"));
    // The theme file is gone, so the link is dangling and `canonicalize` would
    // fail; read the link target itself to prove the old target survived.
    assert_eq!(
        std::fs::read_link(paths.colors_properties()).unwrap(),
        applied
    );
}

#[test]
fn the_list_sits_above_the_preview() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_picker(&mut app);
    let text = render_text(&mut app, 100, 40);
    let list = support::row_of(&text, " themes (").expect("a list pane title");
    let preview = support::row_of(&text, " preview").expect("a preview pane title");
    assert!(
        list < preview,
        "list row {list} is not above preview row {preview}"
    );
    assert_eq!(
        support::column_of(&text, " themes ("),
        support::column_of(&text, " preview"),
        "both panes start at the same column"
    );
    assert!(text.contains("user@termux"), "the preview content is drawn");
    assert!(
        text.contains(".properties"),
        "the preview's last line fits in its pane"
    );
}

#[test]
fn deleting_a_theme_asks_first_and_removes_it() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("T.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let mut app = App::new(paths);
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('d')));
    assert!(app.dialog().is_some(), "delete asks for confirmation");
    assert!(theme.exists(), "nothing is removed before the answer");

    app.resolve_dialog(DialogResult::Confirmed(true));
    assert!(!theme.exists(), "the confirmed theme is gone");
}

#[test]
fn a_cancelled_delete_keeps_the_theme() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("T.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let mut app = App::new(paths);
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('d')));
    app.resolve_dialog(DialogResult::Confirmed(false));
    assert!(theme.exists());
}

#[test]
fn the_active_theme_cannot_be_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("T.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();
    std::os::unix::fs::symlink(&theme, paths.colors_properties()).unwrap();

    let mut app = App::new(paths);
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('d')));
    assert!(app.dialog().is_none());
    assert!(theme.exists());
    assert!(render_text(&mut app, 100, 30).contains("Cannot delete"));
}

#[test]
fn renaming_moves_the_theme_and_keeps_its_content() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("Old.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('R')));
    assert!(app.dialog().is_some(), "rename asks for a name");
    app.resolve_dialog(DialogResult::Text(Some("New".to_owned())));

    let renamed = paths.colors_root().join("New.properties");
    assert!(!theme.exists());
    assert_eq!(
        std::fs::read_to_string(&renamed).unwrap(),
        "background=#000000\n"
    );
}

#[test]
fn renaming_the_active_theme_retargets_the_link() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("Old.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();
    std::os::unix::fs::symlink(&theme, paths.colors_properties()).unwrap();

    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('R')));
    app.resolve_dialog(DialogResult::Text(Some("New".to_owned())));

    assert_eq!(
        std::fs::canonicalize(paths.colors_properties()).unwrap(),
        std::fs::canonicalize(paths.colors_root().join("New.properties")).unwrap()
    );
}

#[test]
fn a_rename_cannot_escape_the_library() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("Old.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('R')));
    app.resolve_dialog(DialogResult::Text(Some("../escape".to_owned())));

    assert!(theme.exists());
    assert!(!dir.path().join("escape.properties").exists());
    assert!(render_text(&mut app, 100, 30).contains("Rename failed"));
}

#[test]
fn deleting_the_last_theme_does_not_reseed_builtins() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("Only.properties");
    std::fs::create_dir_all(paths.colors_root()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let mut app = App::new(paths.clone());
    open_picker(&mut app);
    app.handle_key(key(crossterm::event::KeyCode::Char('d')));
    app.resolve_dialog(DialogResult::Confirmed(true));

    assert!(!theme.exists());
    assert!(!paths.colors_root().join("builtin").exists());
    assert!(render_text(&mut app, 100, 30).contains("No themes found"));
}
