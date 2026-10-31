mod support;

use std::time::{Duration, Instant};

use crossterm::event::KeyCode;
use support::{key, open_fonts, open_picker, render_text};
use termux_settings::app::App;
use termux_settings::paths::Paths;
use termux_settings::runtime::Msg;

#[test]
fn tick_expires_status_within_one_tick() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    app.set_status("Saved thing.");
    assert!(render_text(&mut app, 100, 30).contains("Saved thing."));
    app.update(Msg::Tick(Instant::now() + Duration::from_millis(4250)));
    assert!(!render_text(&mut app, 100, 30).contains("Saved thing."));
}

#[test]
fn small_frames_show_the_guard_and_larger_ones_recover() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    app.update(Msg::Resize(39, 9));
    assert!(render_text(&mut app, 39, 9).contains("too small"));
    assert!(!render_text(&mut app, 100, 30).contains("too small"));
}

#[test]
fn q_quits_from_a_sub_screen() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_picker(&mut app);
    app.handle_key(key(KeyCode::Char('q')));
    assert!(app.should_quit());
}

#[test]
fn msg_prompt_updates_the_fonts_tab() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    open_fonts(&mut app);
    app.update(Msg::PromptCaptured(vec!["$> ".to_string()]));
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("$> "));
}

#[test]
fn reload_failure_shows_hint() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = App::new(Paths::new(dir.path()));
    app.update(Msg::Reloaded(false));
    let rendered = render_text(&mut app, 100, 30);
    assert!(rendered.contains("termux-reload-settings"));
}
