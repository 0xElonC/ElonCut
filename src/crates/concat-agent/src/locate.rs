// SPDX-License-Identifier: AGPL-3.0-or-later
// SPDX-FileCopyrightText: 2026 Jareer Abdullah and Concat contributors

//! Finding the `codex` program.
//!
//! A window started from the Dock or the Start menu does not have the PATH a
//! terminal has, so a search of PATH alone would miss the Codex a person
//! installed with npm or Homebrew. The usual homes are named as well.

use std::path::{Path, PathBuf};

/// The environment variable that names the program outright, for a person
/// whose Codex lives somewhere unusual.
pub const OVERRIDE: &str = "CONCAT_CODEX";

/// The Codex to run, if there is one on this computer.
pub fn codex() -> Option<PathBuf> {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from);
    let path = std::env::var_os("PATH");
    find(
        std::env::var_os(OVERRIDE).map(PathBuf::from),
        path.as_deref(),
        home.as_deref(),
        |candidate| candidate.is_file(),
    )
}

/// [`codex`] over what it reads from the machine, so a test can give it
/// another.
pub fn find(
    forced: Option<PathBuf>,
    path: Option<&std::ffi::OsStr>,
    home: Option<&Path>,
    exists: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    if let Some(forced) = forced.filter(|forced| !forced.as_os_str().is_empty()) {
        return exists(&forced).then_some(forced);
    }
    let name = if cfg!(windows) { "codex.exe" } else { "codex" };
    let mut candidates: Vec<PathBuf> = path
        .map(|path| std::env::split_paths(path).map(|dir| dir.join(name)).collect())
        .unwrap_or_default();
    candidates.extend(
        [
            "/Applications/Codex.app/Contents/Resources/codex",
            "/opt/homebrew/bin/codex",
            "/usr/local/bin/codex",
            "/usr/bin/codex",
        ]
        .map(PathBuf::from),
    );
    if let Some(home) = home {
        for relative in [
            ".codex/plugins/.plugin-appserver/codex",
            ".local/bin/codex",
            ".npm-global/bin/codex",
            ".bun/bin/codex",
            ".volta/bin/codex",
            "AppData/Roaming/npm/codex.cmd",
        ] {
            candidates.push(home.join(relative));
        }
    }
    candidates.into_iter().find(|candidate| exists(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_program_is_the_only_one_tried() {
        let forced = PathBuf::from("/elsewhere/codex");
        assert_eq!(
            find(Some(forced.clone()), None, None, |path| path == forced),
            Some(forced)
        );
        // Named and missing: not a fall back to another, which would run a
        // program the person did not ask for.
        assert_eq!(
            find(Some("/elsewhere/codex".into()), None, None, |_| false),
            None
        );
    }

    #[test]
    fn path_comes_before_the_usual_homes() {
        let on_path = PathBuf::from("/tools/bin").join(if cfg!(windows) {
            "codex.exe"
        } else {
            "codex"
        });
        let found = find(
            None,
            Some(std::ffi::OsStr::new("/tools/bin")),
            Some(Path::new("/home/me")),
            |path| path == on_path || path == Path::new("/home/me/.local/bin/codex"),
        );
        assert_eq!(found, Some(on_path));
    }

    #[test]
    fn the_homes_are_tried_when_path_is_short() {
        let found = find(None, None, Some(Path::new("/home/me")), |path| {
            path == Path::new("/home/me/.codex/plugins/.plugin-appserver/codex")
        });
        assert!(found.is_some());
        assert_eq!(find(None, None, None, |_| false), None);
    }
}
