//! A multi-part path as the engine writes it (#1332).
//!
//! The engine builds paths one `Path::join` at a time, so on Windows they come out
//! `\`-separated. A test that spells its expectation `"a/b/c"` matches on macOS and
//! Linux only. [`native`] turns that spelling into the engine's, the same way on
//! every platform; the `slash-join` lint (`make rules`) flags a `/` literal joined
//! onto a path in test code. [`relative`] spells a folder from the working
//! directory, which a fixed `..` count only does on one platform.

use std::path::{Component, Path, PathBuf};

/// `slashed`, a `/`-separated path, joined one part at a time: what the engine's
/// own `Path::join`s build. A leading `/` stays the root, as `Path::new("/")` is.
pub(crate) fn native(slashed: &str) -> PathBuf {
    let (start, rest) = match slashed.strip_prefix('/') {
        Some(rest) => (PathBuf::from("/"), rest),
        None => (PathBuf::new(), slashed),
    };
    rest.split('/')
        .filter(|part| !part.is_empty())
        .fold(start, |path, part| path.join(part))
}

/// [`native`] as the string the engine writes into a file or a log line.
pub(crate) fn native_string(slashed: &str) -> String {
    native(slashed).to_string_lossy().into_owned()
}

/// `dir` spelled relative to the working directory, the way the editor gives
/// `assets`, so a test can hand the engine a relative root. On Windows a folder on
/// another drive has no relative spelling and stays absolute (CI's checkout is on
/// `D:`, its temp folder on `C:`).
pub(crate) fn relative(dir: &Path) -> PathBuf {
    let anchor = |p: &Path| -> PathBuf {
        let normal = |c: &Component| matches!(c, Component::Normal(_));
        p.components().take_while(|c| !normal(c)).collect()
    };
    let cwd = std::env::current_dir().expect("the working directory");
    if anchor(&cwd) != anchor(dir) {
        return dir.to_path_buf();
    }
    let ups = cwd.strip_prefix(anchor(&cwd)).unwrap().components().count();
    let mut path: PathBuf = std::iter::repeat_n("..", ups).collect();
    path.push(dir.strip_prefix(anchor(dir)).unwrap());
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::MAIN_SEPARATOR_STR as SEP;

    #[test]
    fn a_slashed_path_comes_out_in_the_platforms_separator() {
        assert_eq!(native_string("a/b/c"), ["a", "b", "c"].join(SEP));
        assert_eq!(native("a/b/c"), Path::new("a").join("b").join("c"));
        let rooted = Path::new("/").join("engine").join("target");
        assert_eq!(native("/engine/target"), rooted);
        assert_eq!(
            native("a//b/"),
            Path::new("a").join("b"),
            "empty parts drop"
        );
    }

    #[test]
    fn a_relative_spelling_names_the_same_folder() {
        let dir = crate::test_temp::dir();
        let rel = relative(&dir);
        assert!(rel.is_relative() || cfg!(windows), "another drive: {rel:?}");
        let cwd = std::env::current_dir().unwrap();
        let canonical = |p: &Path| p.canonicalize().unwrap();
        assert_eq!(canonical(&cwd.join(&rel)), canonical(&dir), "{rel:?}");
    }
}
