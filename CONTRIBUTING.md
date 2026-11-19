# Contributing

Thanks for taking a look. This project is small, so the rules are short.

## Requirements

- Rust 1.88.0 or newer (`rust-version` in `Cargo.toml`; CI tests that exact
  toolchain as well as stable). `rust-toolchain.toml` selects stable with
  `rustfmt` and `clippy`; CI overrides the channel per job.
- On Termux: `pkg install rust git`.
- A terminal at least 40×10 for manual testing of the TUI.

## Build

```sh
cargo build
cargo run
```

## Test

```sh
cargo test
```

Tests point the app at a temporary directory through `TERMUX_HOME`, so they
never read or write your real `~/.termux`. Use the same seam when you add a
test:

```rust
let dir = tempfile::tempdir().unwrap();
let paths = Paths::new(dir.path());
```

**Unit tests must not touch the network.** Anything that would need a download
gets its behaviour tested through the pure function instead — for example
`read_capped` takes an `io::Read`, so the cap can be tested without a socket.

When a change is user-visible, add a line to the `Unreleased` section of
`CHANGELOG.md`.

## Lint

Both must be clean before a pull request is reviewed:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo deny check
```

`cargo deny` checks licenses, duplicate versions and dependency sources against
`deny.toml`; install it with `cargo install cargo-deny`.

CI additionally runs `actionlint` on the workflows, `shellcheck` on the scripts
in `scripts/`, `tests/scripts/` and `docs/record-demo.sh`, measures coverage
with `cargo llvm-cov`, and drives the TUI once under a real pty:

```sh
cargo build
sh tests/scripts/tui_smoke_test.sh target/debug/termux-settings
```

## Commit conventions

- [Conventional Commits](https://www.conventionalcommits.org/): `type(scope): summary`
  with `type` one of `feat`, `fix`, `refactor`, `docs`, `ci`, `test`, `chore`.
- Every commit is GPG-signed: `git commit -S`. A ruleset on `main` requires
  signed commits, force-pushes and branch deletion are blocked there, and
  GitHub reports each commit as Verified only if its key is registered on the
  account you push with.
- One logical change per commit; a commit should leave the tests green.
- Commits merged before the ruleset existed (up to 2026-10-30) are unsigned on
  `main`. They are left as they are; rewriting `main` would break every clone.

## Process

1. Write a design in `docs/specs/YYYY-MM-DD-<topic>-design.md`. It records the
   problem, the decisions and the acceptance criteria.
2. Write a plan in `docs/plans/YYYY-MM-DD-<topic>.md`: the task-by-task
   breakdown of that design.
3. Work on a branch, committing as you go, with a test per behaviour change.
4. Open a pull request. CI must pass on the MSRV and stable, and the Android
   build must succeed.
5. Merge with a merge commit. Rebase and squash merges rewrite the branch
   commits and drop their GPG signatures, so the repository no longer accepts
   them; the merge commit GitHub creates is signed and satisfies the ruleset.

`docs/structure.md` explains where each kind of file belongs.
