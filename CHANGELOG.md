# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- The UI now reflows from the terminal's live size on every draw instead of a
  cached one, so showing the on-screen keyboard — or any resize event the app
  missed — can no longer leave a screen laid out for the taller view.
- On short terminals the preview panes give their rows back to the lists: the
  theme list and the terminal-color form are drawn full-height, and the Fonts
  tab drops its prompt preview, then folds the slot into the list title, so the
  font list keeps showing fonts while the keyboard is open.

## [1.3.0] - 2026-10-31

### Security

- Font downloads are restricted to HTTPS GitHub hosts, imported font names
  reject path traversal, and atomic writes refuse to follow a symlink or write
  into `/etc`, `/root`, `/proc` or `/sys`.
- CI grants the `GITHUB_TOKEN` read-only access by default, widening it only in
  the advisory job that files issues and posts check runs.
- The `time` dependency requires 0.3.47 or newer, the release that bounds RFC
  2822 recursion (RUSTSEC-2026-0009), so a regenerated lockfile cannot resolve
  to a vulnerable patch release.

### Changed

- Terminal Colors and Preset Themes stack their panes: the form and the theme
  list now sit above the live preview instead of beside it, so both use the
  full terminal width. The preview keeps its natural height while the pane
  above it scrolls, and on terminals too short for both the preview is dropped
  so the form or list keeps the whole area.

## [1.2.1] - 2026-10-30

### Added

- Release builds publish Termux `.deb` packages for `aarch64` and `arm`,
  attached to the release and uploaded as CI artifacts, so the app can be
  installed with `pkg install ./termux-settings_*.deb` without adding the APT
  repository. The release workflow can also be dispatched manually: a branch run
  only builds and uploads the artifacts, while a run against a tag publishes.

## [1.2.0] - 2026-10-30

### Added

- `--help` and `-h`, printing usage, environment variables and exit codes.
- CI runs the test suite on the MSRV (`1.88.0`) as well as stable, builds the
  Android binaries on every pull request, and scans for RustSec advisories on a
  weekly schedule.
- `dependabot` opens grouped weekly updates for Cargo dependencies and GitHub
  Actions.
- Release tags are gated: the pushed tag must match the crate version, the test
  suite must pass, and the binaries get a build provenance attestation.
- `docs/structure.md`, `CONTRIBUTING.md` and `SECURITY.md`.

### Changed

- The minimum supported Rust version is now `1.88.0`. The previous `1.85.0`
  claim was wrong — the source already used let-chains — and the patched `time`
  release needs 1.88 as well.
- Unknown options, unexpected arguments and options missing their value are now
  usage errors and exit `2` instead of being silently ignored.
- The Nerd Fonts catalog points at upstream `v3.5.1`, with every path
  re-resolved against that tag after upstream flattened its variant directories.
- Dependencies are modernised: `ratatui 0.30`, `crossterm 0.29`,
  `tui-input 0.15` and `ureq 3.5`, and `ttf-parser` is replaced by `skrifa`.
  Nothing user-visible changes; `ttf-parser` is unmaintained and `skrifa` is
  its named successor.
- The release workflow grants write permissions only to the job that attests
  and publishes; the verification job is read-only.
- `main` requires signed commits and rejects force-pushes and deletion. Rebase
  and squash merges are disabled because they rewrite commits and drop their
  GPG signatures.

### Removed

- The public `Paths::backups()` accessor. `Paths::backups_dir()` remains.

### Fixed

- Font downloads are capped at 20 MB, including when the server omits or
  understates `Content-Length`.
- `time` is updated past RUSTSEC-2026-0009 (denial of service via stack
  exhaustion in RFC 2822 parsing).

## [1.1.1] - 2026-10-26

### Added

- MIT licence.

[Unreleased]: https://github.com/GeneralKaos666/termux-settings/compare/v1.3.0...HEAD
[1.3.0]: https://github.com/GeneralKaos666/termux-settings/compare/v1.2.1...v1.3.0
[1.2.1]: https://github.com/GeneralKaos666/termux-settings/compare/v1.2.0...v1.2.1
[1.2.0]: https://github.com/GeneralKaos666/termux-settings/compare/v1.1.1...v1.2.0
[1.1.1]: https://github.com/GeneralKaos666/termux-settings/releases/tag/v1.1.1
