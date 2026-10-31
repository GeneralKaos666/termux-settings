mod support;

use std::time::{Duration, Instant};

use support::{ctrl, open_picker, render_text, row_of};
use termux_settings::app::App;
use termux_settings::paths::Paths;
use termux_settings::runtime::Msg;

#[test]
fn tiny_terminals_get_a_guard_not_a_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    let text = render_text(&mut app, 20, 5);
    assert!(text.contains("too small"));
    app.handle_key(ctrl('c'));
    assert!(app.should_quit());
}

#[test]
fn status_expires_after_four_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    app.set_status("Saved thing.");
    assert!(render_text(&mut app, 100, 30).contains("Saved thing."));
    app.tick(Instant::now() + Duration::from_secs(5));
    assert!(!render_text(&mut app, 100, 30).contains("Saved thing."));
}

#[test]
fn a_resize_does_not_panic() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    for (w, h) in [(10, 4), (40, 10), (120, 40), (39, 9)] {
        let _ = render_text(&mut app, w, h);
    }
}

#[test]
fn a_stale_size_cannot_clip_the_ui() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    // An earlier, larger size is all the app has been told about; the frame is
    // the authority, as it is when the keyboard opens without a resize event.
    app.update(Msg::Resize(100, 40));
    let text = render_text(&mut app, 100, 15);
    assert_eq!(
        row_of(&text, "quit"),
        Some(14),
        "the help bar follows the live size"
    );
}

#[test]
fn a_short_screen_gives_the_rows_to_the_theme_list() {
    let root = tempfile::tempdir().unwrap().keep();
    std::fs::create_dir_all(root.join("colors")).unwrap();
    for name in ["one", "two", "three"] {
        std::fs::write(
            root.join(format!("colors/{name}.properties")),
            "background=#000000\n",
        )
        .unwrap();
    }
    let mut app = App::new(Paths::new(root));
    open_picker(&mut app);
    let text = render_text(&mut app, 100, 15);
    assert!(
        !text.contains(" preview"),
        "the preview gives its rows to the list"
    );
    assert_eq!(row_of(&text, "Tab filter"), Some(14));
    let last_content = text.lines().nth(13).unwrap();
    assert!(
        last_content.starts_with('╰'),
        "the list pane fills the content area: {last_content:?}"
    );
}
