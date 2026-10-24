//! The one temp-repo scaffold the lint tool's tests share (#1387).

use std::fs;
use std::path::{Path, PathBuf};

/// A scratch directory under the temp dir, removed when dropped; a git repository
/// when `git` is set. Its git calls go through [`super::git`], so an outer repository
/// (a hook, `git rebase -x`) is never the one that answers.
pub struct Repo(pub PathBuf);

impl Repo {
    pub fn new(tag: &str, git: bool) -> Repo {
        let dir = std::env::temp_dir().join(format!("rusty-lint-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("temp dir");
        let repo = Repo(dir);
        if git {
            repo.git(&["init", "-q"]);
        }
        repo
    }

    pub fn git(&self, args: &[&str]) {
        run(&self.0, args);
    }

    pub fn file(&self, rel: &str, body: &str) -> PathBuf {
        let path = self.0.join(rel);
        fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        fs::write(&path, body).expect("write");
        path
    }
}

fn run(dir: &Path, args: &[&str]) {
    let ok = super::git(dir).args(args).status().expect("git");
    assert!(ok.success(), "git {args:?}");
}

impl Drop for Repo {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).ok();
    }
}
