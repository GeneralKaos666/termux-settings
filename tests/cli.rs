use std::os::unix::fs::PermissionsExt;
use std::process::ExitCode;

use termux_settings::cli::{Args, run};
use termux_settings::paths::Paths;

#[test]
fn list_fonts_reports_unreadable_library() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let fonts = dir.path().join("fonts");
    std::fs::create_dir_all(&fonts).unwrap();
    std::fs::set_permissions(&fonts, std::fs::Permissions::from_mode(0o000)).unwrap();

    let args = Args {
        list_fonts: true,
        ..Default::default()
    };
    assert!(run(args, &paths).is_err());

    std::fs::set_permissions(&fonts, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn apply_font_unknown_name_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let args = Args {
        apply_font: Some("nope".to_owned()),
        ..Default::default()
    };
    assert_eq!(run(args, &paths).unwrap(), ExitCode::from(2));
}

#[test]
fn unknown_slot_is_rejected() {
    let args = Args::parse_from(["--slot".to_owned(), "bogus".to_owned()]);
    assert!(args.is_err());
}

#[test]
fn help_wins_over_other_flags() {
    let args = Args::parse_from(["--version".into(), "--bogus".into(), "--help".into()]).unwrap();
    assert!(args.help);
    assert!(!args.version);
}

#[test]
fn short_help_alias_is_accepted() {
    assert!(Args::parse_from(["-h".into()]).unwrap().help);
}

#[test]
fn help_text_names_slots_version_and_exit_codes() {
    assert!(termux_settings::cli::HELP.contains(env!("CARGO_PKG_VERSION")));
    assert!(termux_settings::cli::HELP.contains("bold-italic"));
    assert!(termux_settings::cli::HELP.contains("Exit codes"));
}

#[test]
fn unknown_option_is_rejected() {
    let error = Args::parse_from(["--hlep".into()]).unwrap_err();
    assert!(error.contains("unknown option: --hlep"), "{error}");
}

#[test]
fn missing_flag_values_are_rejected() {
    for flag in [
        "--apply-font",
        "--apply-theme",
        "--restore-backup",
        "--slot",
    ] {
        let error = Args::parse_from([flag.to_owned()]).unwrap_err();
        assert!(error.contains(flag), "{error}");
        assert!(error.contains("needs a value"), "{error}");
    }
}

#[test]
fn unexpected_positional_is_rejected() {
    let error = Args::parse_from(["foo".to_owned()]).unwrap_err();
    assert!(error.contains("unexpected argument: foo"), "{error}");
}

#[test]
fn help_run_returns_success() {
    let dir = tempfile::tempdir().unwrap();
    let args = Args {
        help: true,
        ..Default::default()
    };
    assert_eq!(
        run(args, &Paths::new(dir.path())).unwrap(),
        ExitCode::SUCCESS
    );
}

#[test]
fn help_text_names_the_library_and_backup_commands() {
    for flag in [
        "--list-themes",
        "--apply-theme",
        "--list-backups",
        "--restore-backup",
    ] {
        assert!(termux_settings::cli::HELP.contains(flag), "{flag}");
    }
}

#[test]
fn apply_theme_links_colors_properties() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let theme = paths.colors_root().join("dark").join("T.properties");
    std::fs::create_dir_all(theme.parent().unwrap()).unwrap();
    std::fs::write(&theme, "background=#000000\n").unwrap();

    let args = Args {
        apply_theme: Some("dark/T".to_owned()),
        ..Default::default()
    };
    assert_eq!(run(args, &paths).unwrap(), ExitCode::SUCCESS);
    assert_eq!(
        std::fs::canonicalize(paths.colors_properties()).unwrap(),
        std::fs::canonicalize(&theme).unwrap()
    );
}

#[test]
fn apply_theme_unknown_name_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let args = Args {
        apply_theme: Some("nope".to_owned()),
        ..Default::default()
    };
    assert_eq!(
        run(args, &Paths::new(dir.path())).unwrap(),
        ExitCode::from(2)
    );
}

#[test]
fn list_backups_without_a_directory_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let args = Args {
        list_backups: true,
        ..Default::default()
    };
    assert_eq!(
        run(args, &Paths::new(dir.path())).unwrap(),
        ExitCode::SUCCESS
    );
}

#[test]
fn restore_backup_replaces_the_file_it_came_from() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.backups_dir()).unwrap();
    std::fs::write(paths.colors_properties(), "current\n").unwrap();
    std::fs::write(
        paths
            .backups_dir()
            .join("colors-2026-10-25-123456.properties"),
        "restored\n",
    )
    .unwrap();

    let args = Args {
        restore_backup: Some("colors-2026-10-25-123456.properties".to_owned()),
        ..Default::default()
    };
    assert_eq!(run(args, &paths).unwrap(), ExitCode::SUCCESS);
    assert_eq!(
        std::fs::read_to_string(paths.colors_properties()).unwrap(),
        "restored\n"
    );
}

#[test]
fn restore_backup_rejects_names_outside_the_backups_directory() {
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.backups_dir()).unwrap();
    std::fs::write(
        dir.path().join("colors-2026-10-25-123456.properties"),
        "outside\n",
    )
    .unwrap();

    let args = Args {
        restore_backup: Some("../colors-2026-10-25-123456.properties".to_owned()),
        ..Default::default()
    };
    assert_eq!(run(args, &paths).unwrap(), ExitCode::from(2));
    assert!(!paths.colors_properties().exists());
}

#[test]
fn restore_backup_unknown_name_exits_2() {
    let dir = tempfile::tempdir().unwrap();
    let args = Args {
        restore_backup: Some("notes.properties".to_owned()),
        ..Default::default()
    };
    assert_eq!(
        run(args, &Paths::new(dir.path())).unwrap(),
        ExitCode::from(2)
    );
}
