## Summary

<!-- What changes, and why? Link the issue or the spec under docs/specs/. -->

Fixes #

## Changes

<!-- Bullet the user-visible and internal changes. -->

## Test plan

<!-- Commands you ran and what you checked by hand. -->

- [ ] `cargo test --locked`
- [ ] `cargo fmt --check`
- [ ] `cargo clippy --locked --all-targets -- -D warnings`
- [ ] `sh tests/scripts/tui_smoke_test.sh target/debug/termux-settings` (when the TUI loop changed)

## Checklist

- [ ] User-visible changes are listed under `Unreleased` in `CHANGELOG.md`.
- [ ] `README.md` is updated when an interface or key binding changed.
- [ ] New behaviour has a test; bug fixes have a test that fails without the fix.
- [ ] The design and plan live in `docs/specs/` and `docs/plans/` for non-trivial work.
- [ ] Every commit is GPG-signed (`git commit -S`).
