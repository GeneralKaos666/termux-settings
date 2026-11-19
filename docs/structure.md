# Repository structure

`termux-settings` is a single Rust crate: a library (`termux_settings`) holding
all behaviour, and a thin binary (`termux-settings`) that hands `argv` to
`cli::run`. Tests drive the library directly, which is why the binary holds no
logic of its own.

```
.
├── .github/
│   ├── dependabot.yml          weekly grouped cargo + actions updates
│   ├── PULL_REQUEST_TEMPLATE.md  the checklist a PR is reviewed against
│   ├── ISSUE_TEMPLATE/         bug report, feature request and contact links
│   └── workflows/
│       ├── ci.yml              MSRV/stable tests, lint, coverage, deny, Android build, advisory scan
│       └── release.yml         tag gate, Android release build, .deb packages, manual runs, provenance
├── codecov.yml                 coverage reporting and comment layout
├── deny.toml                   cargo-deny licenses, duplicates and sources policy
├── docs/
│   ├── demo.gif                README recording (rebuild with docs/record-demo.sh)
│   ├── record-demo.sh          scripted termtosvg → GIF capture
│   ├── plans/                  implementation plans, one per feature
│   ├── specs/                  design documents, one per feature
│   └── structure.md            this file
├── rust-toolchain.toml         local stable toolchain + rustfmt/clippy (CI selects its own)
├── scripts/
│   ├── build-deb.sh                wraps a release binary in a Termux .deb
│   ├── check-release-version.sh    fails a release tag that disagrees with Cargo.toml
│   └── crate-version.sh            prints the [package] version
├── src/
│   ├── app.rs                  app state, rendering and the message handler
│   ├── chrome.rs               title, borders, footer help bar
│   ├── cli.rs                  argument parsing, `--help`, non-TUI commands, the TUI loop
│   ├── error.rs                the crate error type and its `Result` alias
│   ├── lib.rs                  module wiring and the public surface
│   ├── library.rs              reads `~/.termux/colors/`, activates, renames and deletes themes
│   ├── main.rs                 binary entry point; delegates to `cli::run`
│   ├── paths.rs                every on-disk location, rooted at `TERMUX_HOME`
│   ├── presets.rs              built-in colour presets
│   ├── runtime.rs              the `Msg` channel and the worker spawners
│   ├── terminal.rs             raw mode, the alternate screen and restoration
│   ├── theme.rs                the TUI's own palette, derived from the active theme
│   ├── fonts/
│   │   ├── apply.rs            per-slot font installation and backups
│   │   ├── downloader.rs       Nerd Fonts catalog and capped HTTP downloads
│   │   ├── importer.rs         import a font file from disk
│   │   ├── prompt.rs           the sample prompt used in font previews
│   │   ├── scan.rs             discovers installed fonts, names via `skrifa`
│   │   └── validate.rs         checks that a file is a usable TTF via `skrifa`
│   ├── model/
│   │   ├── app_theme.rs        typed `termux.properties` settings
│   │   ├── colors.rs           the colours model and its serialisation
│   │   └── font_slot.rs        `FontSlot` and slot file names
│   ├── persist/
│   │   ├── backup.rs           timestamped backups, retention, listing and restore
│   │   ├── colors_io.rs        read/write `colors.properties`
│   │   ├── fs.rs               atomic write helper
│   │   ├── properties.rs       line-preserving `termux.properties` editing
│   │   └── theme_io.rs         read/write files under `~/.termux/colors/`
│   ├── screens/
│   │   ├── app_theme_editor.rs App Theme screen
│   │   ├── font_catalog.rs     downloadable Nerd Fonts screen
│   │   ├── fonts_tab.rs        installed font library screen
│   │   ├── main_menu.rs        the main menu
│   │   ├── preset_picker.rs    preset theme browser, rename and delete
│   │   └── terminal_editor.rs  Terminal Colors editor
│   └── ui/
│       ├── dialog.rs           confirmation and text-entry dialogs
│       ├── focus.rs            focus ring tracking
│       ├── form.rs             label/field rows
│       ├── glyphs.rs           ASCII and Unicode glyph sets
│       ├── input.rs            text fields, cursor and filtering
│       └── text.rs             text measuring and truncation
└── tests/
    ├── cli.rs                  argument parsing, exit codes, theme/backup commands
    ├── colors_io.rs            `colors.properties` round-trips
    ├── library.rs              theme library reads
    ├── paths.rs                `TERMUX_HOME` handling
    ├── properties_proptest.rs  generated property files survive an edit
    ├── release_version.rs      the release tag gate against `Cargo.toml`
    ├── roundtrip.rs            property-file round-trips
    ├── scripts/
    │   ├── build_deb_test.sh   package metadata and install path
    │   └── tui_smoke_test.sh   one real-pty run of the TUI
    ├── ui_*.rs                 screen-level behaviour, driven as keystrokes
    ├── fixtures/               golden property files
    └── support/mod.rs          shared test helpers (`TERMUX_HOME` setup)
```

## Where things go

| Path | Purpose |
| --- | --- |
| `.github/workflows/` | CI, release and the weekly advisory scan |
| `.github/ISSUE_TEMPLATE/` | bug and feature forms; security links there go private |
| `deny.toml` | the dependency policy `cargo deny check` enforces |
| `docs/specs/` | design documents: the problem, the decisions, the acceptance criteria |
| `docs/plans/` | implementation plans: the task-by-task breakdown of a spec |
| `scripts/` | repository tooling that CI runs; not installed for users |
| `src/` | the library and binary |
| `tests/` | integration tests, one file per behaviour area |

New work goes spec → plan → branch → PR. The spec records what "done" means;
the plan records how it gets there. Anything that changes user-visible
behaviour updates `CHANGELOG.md` and, when it changes an interface, `README.md`.
