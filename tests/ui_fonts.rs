mod support;

use crossterm::event::KeyCode;
use std::os::unix::fs::PermissionsExt;
use support::{key, minimal_ttf, open_fonts, render_text};
use termux_settings::app::App;
use termux_settings::model::font_slot::FontSlot;
use termux_settings::paths::Paths;
use termux_settings::screens::Screen;
use termux_settings::ui::dialog::DialogResult;

fn fonts_app() -> (App, Paths) {
    let root = tempfile::tempdir().unwrap().keep();
    let paths = Paths::new(root);
    std::fs::create_dir_all(paths.fonts_dir()).unwrap();
    std::fs::write(paths.fonts_dir().join("A.ttf"), minimal_ttf()).unwrap();
    std::fs::write(paths.font_slot_path(FontSlot::Regular), b"orig-regular").unwrap();
    std::fs::write(paths.font_slot_path(FontSlot::Bold), b"orig-bold").unwrap();

    let mut app = App::new(paths.clone());
    open_fonts(&mut app);
    (app, paths)
}

#[test]
fn preview_then_quit_restores() {
    let (mut app, paths) = fonts_app();

    app.handle_key(key(KeyCode::Char('p')));
    app.handle_key(key(KeyCode::Char('s')));
    app.handle_key(key(KeyCode::Char('p')));

    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Regular)).unwrap(),
        minimal_ttf()
    );
    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Bold)).unwrap(),
        minimal_ttf()
    );

    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit());

    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Regular)).unwrap(),
        b"orig-regular"
    );
    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Bold)).unwrap(),
        b"orig-bold"
    );
}

#[test]
fn esc_restores_then_pops() {
    let (mut app, paths) = fonts_app();

    app.handle_key(key(KeyCode::Char('p')));
    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Regular)).unwrap(),
        minimal_ttf()
    );

    app.handle_key(key(KeyCode::Esc));
    assert_eq!(
        std::fs::read(paths.font_slot_path(FontSlot::Regular)).unwrap(),
        b"orig-regular"
    );
    assert!(matches!(app.current(), Screen::Main(_)));
}

#[test]
fn filter_narrows_the_list_and_esc_clears() {
    let root = tempfile::tempdir().unwrap().keep();
    let paths = Paths::new(root);
    std::fs::create_dir_all(paths.fonts_dir()).unwrap();
    std::fs::write(paths.fonts_dir().join("A.ttf"), minimal_ttf()).unwrap();
    std::fs::write(paths.fonts_dir().join("B.ttf"), minimal_ttf()).unwrap();
    let mut app = App::new(paths);
    open_fonts(&mut app);

    app.handle_key(key(KeyCode::Char('/')));
    app.handle_key(key(KeyCode::Char('B')));
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("B.ttf"));
    assert!(!rendered.contains("A.ttf"));

    app.handle_key(key(KeyCode::Esc));
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("A.ttf"));
}

#[test]
fn import_flow_offers_clash_dialog() {
    let root = tempfile::tempdir().unwrap().keep();
    let paths = Paths::new(root.clone());
    std::fs::create_dir_all(paths.fonts_dir()).unwrap();
    std::fs::write(paths.fonts_dir().join("A.ttf"), minimal_ttf()).unwrap();
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    let src = src_dir.join("A.ttf");
    std::fs::write(&src, minimal_ttf()).unwrap();

    let mut app = App::new(paths.clone());
    open_fonts(&mut app);

    app.handle_key(key(KeyCode::Char('i')));
    assert!(app.dialog().is_some());

    app.resolve_dialog(DialogResult::Text(Some(src.to_string_lossy().into_owned())));
    assert!(app.dialog().is_some());
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("already in library"));

    app.resolve_dialog(DialogResult::Confirmed(true));
    assert!(paths.fonts_dir().join("A-1.ttf").exists());
}

#[test]
fn a_short_screen_keeps_the_font_rows() {
    let root = tempfile::tempdir().unwrap().keep();
    let paths = Paths::new(root);
    std::fs::create_dir_all(paths.fonts_dir()).unwrap();
    for name in ["A", "B", "C", "D", "E", "F"] {
        std::fs::write(paths.fonts_dir().join(format!("{name}.ttf")), minimal_ttf()).unwrap();
    }
    let mut app = App::new(paths);
    open_fonts(&mut app);

    let text = render_text(&mut app, 100, 15);
    assert!(
        !text.contains(" Preview"),
        "the prompt preview gives its rows to the list"
    );
    assert!(text.contains("F.ttf"), "the list keeps its rows:\n{text}");

    let text = render_text(&mut app, 100, 11);
    assert!(
        text.contains("slot"),
        "the slot line folds into the list title:\n{text}"
    );
}

#[test]
fn unreadable_library_shows_status() {
    let root = tempfile::tempdir().unwrap().keep();
    let paths = Paths::new(root);
    std::fs::create_dir_all(paths.fonts_dir()).unwrap();
    std::fs::set_permissions(paths.fonts_dir(), std::fs::Permissions::from_mode(0o000)).unwrap();

    let mut app = App::new(paths.clone());
    open_fonts(&mut app);
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("unreadable"));

    std::fs::set_permissions(paths.fonts_dir(), std::fs::Permissions::from_mode(0o755)).unwrap();
}
