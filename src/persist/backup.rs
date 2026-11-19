//! Timestamped backups of Termux config files.
//!
//! Before the editor overwrites `colors.properties` or `termux.properties`,
//! [`make_backup`] copies the current file into a backups directory under a
//! timestamped name so a mistaken save can be undone. Symlinks are followed so
//! a backup holds the real colour data rather than a dangling link.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use time::error::IndeterminateOffset;
use time::{OffsetDateTime, UtcOffset};

use crate::error::Result;
use crate::persist::fs::write_atomic;

/// How many backups of one kind are kept; older ones are pruned.
pub const BACKUPS_KEPT: usize = 10;

/// A completed backup: where it was written and whether UTC was used.
#[derive(Debug)]
pub struct Backup {
    /// The path the backup was written to.
    pub path: PathBuf,
    /// Whether the local offset could not be determined and UTC was used.
    pub utc_fallback: bool,
}

/// Formats `at` as `%Y-%m-%d-%H%M%S`, optionally converting to `offset` first.
///
/// When `offset` is `Some`, `at` is reinterpreted in that offset before
/// formatting; `None` formats `at` as given (already in the desired offset).
pub fn format_timestamp(at: OffsetDateTime, offset: Option<UtcOffset>) -> String {
    let at = match offset {
        Some(offset) => at.to_offset(offset),
        None => at,
    };
    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        at.year(),
        at.month() as u8,
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
    )
}

/// Copies `source` into `backups_dir` under a timestamped name.
///
/// Returns `Ok(None)` when `source` does not exist. Otherwise the source bytes
/// are read (following symlinks) and written to
/// `<backups_dir>/<prefix>-<stamp>.properties`, where `<stamp>` is the current
/// local time formatted by [`format_timestamp`]. The directory is created if
/// needed. On a name collision the suffix `-1`, `-2`, … is appended until the
/// destination does not exist. The stamp is captured once per call; when the
/// local offset cannot be determined the time falls back to UTC and
/// [`Backup::utc_fallback`] is set.
pub fn make_backup(source: &Path, backups_dir: &Path, prefix: &str) -> Result<Option<Backup>> {
    make_unique_backup(source, backups_dir, prefix, ".properties")
}

/// Whether `name` is `<stem>-<YYYY-MM-DD-HHMMSS><ext>`.
///
/// The stamp is exactly 17 characters with dashes at offsets 4, 7 and 10 and
/// digits everywhere else, so a file like `font-2026-10-25-123456.ttf` is
/// recognised while a short or misplaced-dash name is not.
pub fn is_stamped_name(name: &str, stem: &str, ext: &str) -> bool {
    let prefix = format!("{stem}-");
    if !name.starts_with(&prefix) || !name.ends_with(ext) {
        return false;
    }
    let middle = &name[prefix.len()..name.len() - ext.len()];
    if middle.len() != 17 {
        return false;
    }
    middle.chars().enumerate().all(|(index, c)| {
        if matches!(index, 4 | 7 | 10) {
            c == '-'
        } else {
            c.is_ascii_digit()
        }
    })
}

/// Copies `source` into `dir` under a timestamped name with extension `ext`.
///
/// See [`make_backup`]; identical except the caller chooses the extension
/// (including the leading dot, e.g. `".ttf"`). After writing, only the newest
/// [`BACKUPS_KEPT`] backups of this prefix and extension are kept.
pub fn make_unique_backup(
    source: &Path,
    dir: &Path,
    prefix: &str,
    ext: &str,
) -> Result<Option<Backup>> {
    if !source.exists() {
        return Ok(None);
    }
    std::fs::create_dir_all(dir)?;
    let bytes = std::fs::read(source)?;

    let (at, utc_fallback) = match OffsetDateTime::now_local() {
        Ok(local) => (local, false),
        Err(IndeterminateOffset) => (OffsetDateTime::now_utc(), true),
    };
    let stamp = format_timestamp(at, None);

    let mut destination = dir.join(format!("{prefix}-{stamp}{ext}"));
    let mut counter = 1;
    while destination.exists() {
        destination = dir.join(format!("{prefix}-{stamp}-{counter}{ext}"));
        counter += 1;
    }
    std::fs::write(&destination, bytes)?;
    prune_backups(dir, prefix, ext);

    Ok(Some(Backup {
        path: destination,
        utc_fallback,
    }))
}

/// The stamp and collision counter encoded in a backup file name.
///
/// Accepts `<stem>-<YYYY-MM-DD-HHMMSS><ext>` and the collision form with a
/// `-<n>` suffix. Returns `None` for any other name, so a pruning pass can
/// never touch a user file that merely shares the prefix.
fn backup_parts<'a>(name: &'a str, stem: &str, ext: &str) -> Option<(&'a str, u64)> {
    let prefix = format!("{stem}-");
    if !name.starts_with(&prefix) || !name.ends_with(ext) {
        return None;
    }
    let middle = &name[prefix.len()..name.len().checked_sub(ext.len())?];
    if middle.len() < 17 {
        return None;
    }
    let (stamp, rest) = middle.split_at(17);
    let stamp_ok = stamp.char_indices().all(|(index, c)| {
        if matches!(index, 4 | 7 | 10) {
            c == '-'
        } else {
            c.is_ascii_digit()
        }
    });
    if !stamp_ok {
        return None;
    }
    let ordinal = if rest.is_empty() {
        0
    } else {
        rest.strip_prefix('-')?.parse().ok()?
    };
    Some((stamp, ordinal))
}

/// Whether `name` is a backup of `stem` with extension `ext`, including the
/// `-<n>` collision suffix.
pub fn is_backup_name(name: &str, stem: &str, ext: &str) -> bool {
    backup_parts(name, stem, ext).is_some()
}

/// Deletes all but the newest [`BACKUPS_KEPT`] backups of `stem`/`ext` in `dir`.
///
/// Names are ordered by their timestamp and collision counter, not by file
/// modification time, so a copied or restored file cannot jump the queue.
fn prune_backups(dir: &Path, stem: &str, ext: &str) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut matching: Vec<(String, u64, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let (stamp, ordinal) = backup_parts(&name, stem, ext)?;
            Some((stamp.to_owned(), ordinal, entry.path()))
        })
        .collect();
    matching.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    for (_, _, path) in matching.into_iter().skip(BACKUPS_KEPT) {
        let _ = std::fs::remove_file(path);
    }
}

/// Copies backup `source` over `target`, backing up the current target first.
///
/// `source` must be a direct child of `dir` and must match the `<prefix>-…`
/// backup naming scheme, so a crafted argument cannot read an arbitrary file.
/// Returns the backup of the replaced file, when there was one.
pub fn restore_backup(
    source: &Path,
    target: &Path,
    dir: &Path,
    prefix: &str,
    ext: &str,
) -> Result<Option<Backup>> {
    let name = source
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| crate::error::Error::Invalid("backup name is not valid UTF-8".to_owned()))?;
    if source.parent() != Some(dir) || !is_backup_name(name, prefix, ext) {
        return Err(crate::error::Error::Invalid(format!(
            "not a {prefix} backup: {name}"
        )));
    }
    let bytes = std::fs::read(source)?;
    let previous = make_unique_backup(target, dir, prefix, ext)?;
    write_atomic(target, &bytes)?;
    Ok(previous)
}

/// Lists the regular files in `dir`, newest first by modification time.
///
/// Returns an empty vector when `dir` is missing.
pub fn list_backups(dir: &Path) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files: Vec<(SystemTime, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        files.push((entry.metadata()?.modified()?, path));
    }
    files.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
    Ok(files.into_iter().map(|(_, path)| path).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_in_the_given_offset() {
        let at = OffsetDateTime::from_unix_timestamp(1_767_225_600).unwrap(); // 2026-01-01T00:00:00Z
        let offset = UtcOffset::from_hms(-8, 0, 0).unwrap();
        assert_eq!(format_timestamp(at, Some(offset)), "2025-12-31-160000");
        assert_eq!(format_timestamp(at, None), "2026-01-01-000000");
    }

    #[test]
    fn copies_content_and_follows_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("Theme.properties");
        std::fs::write(&target, "foreground=#ffffff\n").unwrap();
        let link = dir.path().join("colors.properties");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        let backups = dir.path().join("backups");
        let backup = make_backup(&link, &backups, "colors").unwrap().unwrap();

        assert!(backup.path.starts_with(&backups));
        assert!(
            backup
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("colors-")
        );
        assert_eq!(
            std::fs::read_to_string(&backup.path).unwrap(),
            "foreground=#ffffff\n"
        );
    }

    #[test]
    fn collisions_get_a_counter_suffix() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("colors.properties");
        std::fs::write(&source, "a=1\n").unwrap();
        let backups = dir.path().join("backups");
        let first = make_backup(&source, &backups, "colors")
            .unwrap()
            .unwrap()
            .path;
        let second = make_backup(&source, &backups, "colors")
            .unwrap()
            .unwrap()
            .path;
        assert_ne!(first, second);
        assert!(
            second
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("-1.properties")
        );
    }

    #[test]
    fn absent_source_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            make_backup(&dir.path().join("nope"), &dir.path().join("b"), "x")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn lists_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("old"), b"x").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        std::fs::write(dir.path().join("new"), b"x").unwrap();
        let listed = list_backups(dir.path()).unwrap();
        assert_eq!(listed[0].file_name().unwrap(), "new");
    }

    #[test]
    fn stamped_name_accepts_17_char_stamp() {
        assert!(is_stamped_name(
            "colors-2026-10-25-123456.properties",
            "colors",
            ".properties"
        ));
        assert!(is_stamped_name(
            "font-2026-10-25-123456.ttf",
            "font",
            ".ttf"
        ));
    }

    #[test]
    fn stamped_name_rejects_wrong_length() {
        assert!(!is_stamped_name(
            "colors-2026-10-25-12345.properties",
            "colors",
            ".properties"
        ));
        assert!(!is_stamped_name(
            "colors-2026-10-25.properties",
            "colors",
            ".properties"
        ));
    }

    #[test]
    fn stamped_name_rejects_misplaced_dashes() {
        assert!(!is_stamped_name(
            "colors-2026-1-25-123456.properties",
            "colors",
            ".properties"
        ));
        assert!(!is_stamped_name(
            "colors-2026-10-25-12345x.properties",
            "colors",
            ".properties"
        ));
    }

    #[test]
    fn make_unique_backup_writes_custom_ext() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("font.ttf");
        std::fs::write(&source, b"fontbytes").unwrap();
        let backups = dir.path().join("backups");
        let backup = make_unique_backup(&source, &backups, "font", ".ttf")
            .unwrap()
            .unwrap();
        let name = backup
            .path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();
        assert!(name.starts_with("font-") && name.ends_with(".ttf"));
        assert_eq!(std::fs::read(&backup.path).unwrap(), b"fontbytes");
    }

    #[test]
    fn keeps_only_the_newest_backups() {
        let dir = tempfile::tempdir().unwrap();
        for ordinal in 0..6 {
            std::fs::write(
                dir.path()
                    .join(format!("colors-2026-01-01-000000-{ordinal}.properties")),
                b"old",
            )
            .unwrap();
            std::fs::write(
                dir.path()
                    .join(format!("colors-2026-01-02-000000-{ordinal}.properties")),
                b"new",
            )
            .unwrap();
        }
        prune_backups(dir.path(), "colors", ".properties");

        let mut names: Vec<String> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), BACKUPS_KEPT);
        assert!(
            !names
                .iter()
                .any(|name| name.contains("2026-01-01-000000-0."))
        );
        assert!(
            !names
                .iter()
                .any(|name| name.contains("2026-01-01-000000-1."))
        );
        assert!(is_backup_name(
            "colors-2026-10-25-123456-2.properties",
            "colors",
            ".properties"
        ));
    }

    #[test]
    fn restore_replaces_the_target_and_keeps_the_old_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        let target = dir.path().join("colors.properties");
        std::fs::write(&target, b"current").unwrap();
        let source = backups.join("colors-2026-10-25-123456.properties");
        std::fs::write(&source, b"restored").unwrap();

        let previous = restore_backup(&source, &target, &backups, "colors", ".properties")
            .unwrap()
            .expect("the current file is backed up");
        assert_eq!(std::fs::read(&target).unwrap(), b"restored");
        assert_eq!(std::fs::read(&previous.path).unwrap(), b"current");
    }

    #[test]
    fn restore_refuses_a_file_outside_the_backup_scheme() {
        let dir = tempfile::tempdir().unwrap();
        let backups = dir.path().join("backups");
        std::fs::create_dir_all(&backups).unwrap();
        let target = dir.path().join("colors.properties");
        let outside = dir.path().join("colors-2026-10-25-123456.properties");
        std::fs::write(&outside, b"nope").unwrap();

        assert!(
            restore_backup(&outside, &target, &backups, "colors", ".properties").is_err(),
            "a backup must live in the backups directory"
        );
        assert!(
            restore_backup(
                &backups.join("notes.properties"),
                &target,
                &backups,
                "colors",
                ".properties"
            )
            .is_err(),
            "an unrelated file name is not a colors backup"
        );
    }
}
