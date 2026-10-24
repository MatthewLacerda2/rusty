//! A folder spelled from the working directory (#1332).
//!
//! [`relative`] spells a folder from the working directory, which a fixed `..`
//! count only does on one platform. A multi-part path as the engine writes it is
//! `core::paths::native`, which the engine itself uses (#1371).

use std::path::{Component, Path, PathBuf};

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
