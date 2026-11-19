//! Scan the theme library and activate themes by symlink.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::model::colors::TerminalColors;
use crate::paths::Paths;
use crate::persist::colors_io::read_colors;

/// How often [`reload_with_path_env`] checks whether the process exited.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

/// How long [`reload_with_path_env`] waits before giving up and killing the
/// child, matching the Python implementation's `subprocess.run(timeout=15)`.
const RELOAD_TIMEOUT: Duration = Duration::from_secs(15);

/// One theme file discovered in the colour library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThemeRef {
    /// The theme's file name without its `.properties` extension.
    pub name: String,
    /// The full path to the theme file.
    pub path: PathBuf,
    /// The library sub-directory holding the theme, or `"root"` when it sits
    /// directly under the library root.
    pub group: String,
}

impl ThemeRef {
    /// The user-facing label: `group/name` inside a directory, `name` at root.
    pub fn label(&self) -> String {
        if self.group != "root" {
            format!("{}/{}", self.group, self.name)
        } else {
            self.name.clone()
        }
    }
}

/// Whether a file name marks a backup, matching the Python `_is_backup`.
fn is_backup(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("backup") || name.ends_with(".bak") || name.contains(".bak-")
}

/// The top-level library sub-directory holding `path`, or `"root"`.
///
/// Mirrors the Python authority, which takes the first path component relative
/// to the root rather than the immediate parent, so a nested theme such as
/// `dark/sub/Foo.properties` still groups under `dark`.
fn theme_group(root: &Path, path: &Path) -> String {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let mut parts = relative.components();
    let Some(first) = parts.next() else {
        return "root".to_owned();
    };
    if parts.next().is_none() {
        return "root".to_owned();
    }
    first.as_os_str().to_string_lossy().into_owned()
}

/// Walks `dir` recursively, pushing every usable theme under `root` into
/// `themes`. Unreadable directories are skipped silently.
fn collect_themes(root: &Path, dir: &Path, themes: &mut Vec<ThemeRef>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        if path.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            collect_themes(root, &path, themes);
        } else if (path.is_file() || path.is_symlink())
            && name.ends_with(".properties")
            && name != "colors.properties"
            && !is_backup(&name)
        {
            let name = path
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let group = theme_group(root, &path);
            themes.push(ThemeRef { name, path, group });
        }
    }
}

/// Finds every usable `*.properties` theme under `root`.
///
/// The active `colors.properties` file, backups, and dotfiles are skipped.
/// Results are ordered by group then name, with the root group last.
pub fn scan_themes(root: &Path) -> Vec<ThemeRef> {
    let mut themes = Vec::new();
    collect_themes(root, root, &mut themes);
    themes.sort_by_key(|theme| {
        (
            theme.group == "root",
            theme.group.to_lowercase(),
            theme.name.to_lowercase(),
        )
    });
    themes
}

/// Returns the real theme file `colors_properties` resolves to, if it is a
/// symlink.
pub fn active_theme_path(colors_properties: &Path) -> Option<PathBuf> {
    if !colors_properties.is_symlink() {
        return None;
    }
    std::fs::canonicalize(colors_properties).ok()
}

/// Returns the colours currently configured, or the defaults when unset.
pub fn active_colors(paths: &Paths) -> TerminalColors {
    read_colors(&paths.colors_properties()).unwrap_or_default()
}

/// Points `colors_properties` at `theme` by creating a fresh symlink.
///
/// An existing file or symlink is replaced, and the caller is responsible for
/// backing it up first (parity with the Python implementation, which does not
/// back up here). When `theme` and `colors_properties` already resolve to the
/// same file — including the self-link case where the user edits
/// `colors.properties` in place — this is a no-op.
pub fn apply_theme(theme: &Path, colors_properties: &Path) -> Result<()> {
    let resolved =
        std::fs::canonicalize(theme).map_err(|_| Error::NotFound(theme.to_path_buf()))?;
    if !resolved.is_file() {
        return Err(Error::Invalid(format!(
            "theme is not a file: {}",
            theme.display()
        )));
    }
    if let Ok(existing) = std::fs::canonicalize(colors_properties)
        && existing == resolved
    {
        return Ok(());
    }
    if let Ok(meta) = std::fs::symlink_metadata(colors_properties)
        && meta.file_type().is_dir()
    {
        return Err(Error::Invalid(format!(
            "{} is a directory",
            colors_properties.display()
        )));
    }
    let dir = colors_properties
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let file_name = colors_properties
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "colors.properties".to_owned());
    let tmp = dir.join(format!(".{file_name}.tmp-{}", std::process::id()));

    // Create the new link under a temp name, then atomically rename it over the
    // destination, so a failure never leaves `colors.properties` missing or
    // pointing at nothing.
    if let Err(error) = std::os::unix::fs::symlink(&resolved, &tmp) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    if let Err(error) = std::fs::rename(&tmp, colors_properties) {
        let _ = std::fs::remove_file(&tmp);
        return Err(error.into());
    }
    Ok(())
}

/// The theme whose label or bare name matches `name`, ignoring case.
///
/// `name` may be `group/theme` (the label) or just `theme`; the first match in
/// library order wins.
pub fn resolve_theme(root: &Path, name: &str) -> Option<ThemeRef> {
    let needle = name.to_lowercase();
    scan_themes(root)
        .into_iter()
        .find(|theme| theme.label().to_lowercase() == needle || theme.name.to_lowercase() == needle)
}

/// Returns the `.properties` stem `new_name` asks for, or an error when it is
/// not a plain file name.
fn theme_stem(new_name: &str) -> Result<&str> {
    let stem = new_name.trim();
    let stem = stem.strip_suffix(".properties").unwrap_or(stem);
    if stem.is_empty() || stem == "." || stem == ".." {
        return Err(Error::Invalid("theme name is empty".to_owned()));
    }
    if stem.starts_with('.') {
        return Err(Error::Invalid(
            "theme name cannot start with a dot".to_owned(),
        ));
    }
    if stem.contains('/') || stem.contains('\\') || stem.contains('\0') {
        return Err(Error::Invalid(
            "theme name cannot contain path separators".to_owned(),
        ));
    }
    Ok(stem)
}

/// Renames the theme file at `theme` to `<new_name>.properties` in the same
/// directory, returning the new path.
///
/// `termux.properties`-style backups and other files are untouched. The caller
/// must retarget `colors.properties` when the renamed theme is the active one.
pub fn rename_theme(theme: &Path, new_name: &str) -> Result<PathBuf> {
    let stem = theme_stem(new_name)?;
    let parent = theme
        .parent()
        .ok_or_else(|| Error::Invalid("theme has no parent directory".to_owned()))?;
    let destination = parent.join(format!("{stem}.properties"));
    if destination == theme {
        return Ok(destination);
    }
    if !theme.is_file() {
        return Err(Error::NotFound(theme.to_path_buf()));
    }
    if destination.exists() {
        return Err(Error::Invalid(format!(
            "a theme named {stem:?} already exists"
        )));
    }
    std::fs::rename(theme, &destination)?;
    Ok(destination)
}

/// Deletes the theme file at `theme`.
///
/// Removing the active theme is refused by the caller, not here; this only
/// unlinks the file so a backup or a still-linked `colors.properties` can be
/// handled deliberately.
pub fn delete_theme(theme: &Path) -> Result<()> {
    if !theme.is_file() {
        return Err(Error::NotFound(theme.to_path_buf()));
    }
    std::fs::remove_file(theme)?;
    Ok(())
}

/// Runs `termux-reload-settings` when it is on the process `PATH`.
pub fn reload_termux() -> bool {
    reload_with_path_env(None)
}

/// Like [`reload_termux`], but searches `path_env` instead of the process
/// `PATH`. `None` uses the current `PATH`. Exposed for tests so they can point
/// at a `PATH` without touching the process environment.
#[doc(hidden)]
pub fn reload_with_path_env(path_env: Option<&str>) -> bool {
    let Some(executable) = find_executable("termux-reload-settings", path_env) else {
        return false;
    };
    let Ok(mut child) = Command::new(&executable).spawn() else {
        return false;
    };
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) => {
                if start.elapsed() >= RELOAD_TIMEOUT {
                    let _ = child.kill();
                    let _ = child.wait();
                    return false;
                }
                std::thread::sleep(POLL_INTERVAL);
            }
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

/// Searches `path_env` (or the process `PATH` when `None`) for `name`.
fn find_executable(name: &str, path_env: Option<&str>) -> Option<PathBuf> {
    let path_var = match path_env {
        Some(paths) => paths.to_owned(),
        None => std::env::var_os("PATH")?.to_string_lossy().into_owned(),
    };
    for dir in path_var.split(':') {
        let candidate = Path::new(dir).join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
