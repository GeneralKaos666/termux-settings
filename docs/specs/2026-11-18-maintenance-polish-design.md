# Maintenance polish: design

- **Status:** approved (design)
- **Date:** 2026-11-18
- **Scope:** repository polish after `1.3.1`
- **Applies to:** version `1.3.1` and later

## Context

An audit of `1.3.1` found the codebase healthy — 225 tests, clean `fmt` and
`clippy`, an MSRV matrix, Android builds, advisory scanning, specs and plans —
with concrete gaps outside that core:

1. `CONTRIBUTING.md` promises `actionlint` and `shellcheck` in CI; neither runs.
2. Only fonts have non-TUI commands. Themes, the main feature, cannot be listed
   or applied from a script.
3. Backups are write-only: `list_backups` exists but no user-facing path calls
   it, and `~/.termux/backups/` grows without limit.
4. The README does not link `CONTRIBUTING.md`, `SECURITY.md` or
   `docs/structure.md`, omits the published crate, and has no visual.
5. No coverage measurement, no license/duplicate/source policy, no generated
   test for the line-preservation promise, and no test of raw-mode entry.
6. No issue forms, no PR template and no contributor toolchain pin.

## Goals

- CI enforces what the contributor guide says it enforces.
- Themes and backups are manageable from the command line as well as the TUI.
- A theme can be renamed or deleted without leaving a dangling
  `colors.properties` link.
- Backup growth is bounded, and every backup can be restored.
- A contributor can find the demo, the contribution rules, the security route,
  the module map and the dependency policy from the README.
- The TUI's real terminal lifecycle is exercised once per CI run.

## Non-goals

- Changing the colours, app-theme or font on-disk formats.
- New editors or screens beyond library rename/delete.
- A coverage floor; coverage is reported and archived, not gated.
- Publishing to crates.io from CI. The crate is published manually today; the
  missing token would fail every release.

## Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| Theme commands | `--list-themes`, `--apply-theme <name>` | Mirrors the font commands, reuses `library::apply_theme`, and keeps the hand-written parser. |
| Backup commands | `--list-backups`, `--restore-backup <name>` | The data already exists; a name, not a path, keeps traversal out and matches what listing prints. |
| Retention | Keep the ten newest per prefix/extension | Bounded growth without asking the user to configure anything. Ordering uses the timestamp in the name so a copied file cannot jump the queue. |
| Restore safety | Back up the current file, then write atomically | A restore is itself undoable, and a crash cannot truncate the target. |
| Active theme | Refuse delete, retarget on rename | Deleting would leave a dangling link; retargeting keeps the active colours. |
| CI gates | Lint, coverage, deny, pty smoke | Each covers a failure the existing jobs cannot see. Deny runs on advisories, licenses, duplicates and sources. |
| Demo recording | `termtosvg` → `rsvg-convert` → GIF | VHS needs Chromium, which Termux does not ship; the still-frame path uses tools already present. |
| Toolchain file | Pin `1.88.0` with `rustfmt` and `clippy` | Matches the MSRV CI already treats as the compatibility gate. `RUSTUP_TOOLCHAIN` from CI still selects stable. |

## Acceptance criteria

- `termux-settings --list-themes` prints `group/name` labels; `--apply-theme`
  resolves either the label or the bare name, ignoring case, and exits `2` for
  an unknown theme.
- `--list-backups` prints names newest first; `--restore-backup` restores a
  `colors-*`, `termux-*` or font-slot backup, refuses path-like names with exit
  `2`, and leaves a backup of the replaced file.
- Eleven backups of one kind leave ten; the oldest is gone.
- `d` asks before deleting, refuses the active theme, and `R` asks for a name,
  rejects path separators, and retargets the link when the renamed theme is
  active.
- `cargo deny check`, `actionlint` and `shellcheck` pass; coverage and a pty
  smoke test run on every pull request.
- The property tests pass for generated comment, continuation and unknown-key
  lines.
- README shows the demo and links the contributing, security and structure
  documents.
