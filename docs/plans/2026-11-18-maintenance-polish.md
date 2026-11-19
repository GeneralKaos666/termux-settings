# Maintenance polish: plan

- **Spec:** `docs/specs/2026-11-18-maintenance-polish-design.md`
- **Date:** 2026-11-18

## Tasks

1. **Library helpers.** Add `resolve_theme`, `rename_theme` and `delete_theme`
   to `src/library.rs`; reject empty, dotted and path-like names.
2. **Backup retention and restore.** In `src/persist/backup.rs`, add
   `is_backup_name` (stamp plus collision suffix), prune to
   `BACKUPS_KEPT = 10` after a write, and add `restore_backup` that validates
   the name and backs up the target before the atomic overwrite.
3. **CLI parity.** Add `--list-themes`, `--apply-theme`, `--list-backups` and
   `--restore-backup`; update `HELP`, exit codes and `tests/cli.rs`.
4. **Theme management in the picker.** `d` opens a confirm dialog, `R` a prompt;
   `on_dialog` performs the change, reloads the library and retargets
   `colors.properties` when the active theme is renamed. Cover delete, cancel,
   active refusal, rename, active retarget and traversal in `tests/ui_picker.rs`.
5. **Property tests.** Add `proptest` as a dev-dependency and
   `tests/properties_proptest.rs` for line preservation, idempotence and
   unset line count.
6. **PTY smoke test.** Add `tests/scripts/tui_smoke_test.sh`, run it from the CI
   test job after `cargo build`.
7. **CI.** Add a lint job (`actionlint`, `shellcheck`), a coverage job
   (`cargo llvm-cov`, lcov artifact, step summary), a `cargo deny` job, a
   concurrency group, per-job timeouts, and `taiki-e/install-action` for
   `cargo-ndk` in both workflows.
8. **Policy and tooling.** Add `deny.toml` and `rust-toolchain.toml`.
9. **Community files.** Add `.github/ISSUE_TEMPLATE/{bug_report,feature_request,config}.yml`
   and `.github/PULL_REQUEST_TEMPLATE.md`.
10. **Demo and docs.** Record `docs/demo.gif` with `docs/record-demo.sh`, embed
    it in `README.md`, update the install, key, CLI, backup and development
    sections, and refresh `CONTRIBUTING.md`, `docs/structure.md` and
    `CHANGELOG.md`.

## Verification

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo deny check
actionlint
shellcheck scripts/*.sh tests/scripts/*.sh docs/record-demo.sh
cargo build && sh tests/scripts/tui_smoke_test.sh target/debug/termux-settings
```
