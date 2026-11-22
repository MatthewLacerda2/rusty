use std::fs;
use std::path::{Path, PathBuf};

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
