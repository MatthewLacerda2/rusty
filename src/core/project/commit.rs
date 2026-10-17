//! src/core/project/commit.rs — which engine commit this binary is (#853, #931).
//!
//! A **release** build has the commit stamped in at compile time (`build.rs` sets
//! `RUSTY_ENGINE_COMMIT`), because a shipped binary has no checkout to ask. A **debug**
//! build asks git at run time, in the checkout it was built from: stamping it there
//! would change the crate's fingerprint on every commit, checkout and rebase, and
//! rebuild the whole crate once per feature set (#931 measured ~15 s each).
//!
//! The file is std-only: `build.rs` includes it with `#[path]`, so both read git the
//! same way.

use std::path::Path;
// lint: allow(subprocess) asks git which commit a debug build's checkout is; a release build is stamped and never runs it
use std::process::Command;

/// The commit of a build that can't say which commit it is (no git, no checkout).
pub const UNKNOWN_COMMIT: &str = "unknown";

/// The engine commit this binary is: a full git hash, `-dirty` when the tree had
/// changes, or [`UNKNOWN_COMMIT`]. Release builds report the commit they were built
/// from; debug builds report the checkout's commit, read once per process.
pub fn engine_commit() -> &'static str {
    static COMMIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    COMMIT.get_or_init(|| match option_env!("RUSTY_ENGINE_COMMIT") {
        Some(stamped) => stamped.to_string(),
        None => git_commit(Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|| UNKNOWN_COMMIT.to_string()),
    })
}

/// `git rev-parse HEAD` in `dir`, suffixed `-dirty` when tracked files had changes;
/// `None` without git or outside a repository.
pub fn git_commit(dir: &Path) -> Option<String> {
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    let dirty =
        git(dir, &["status", "--porcelain", "--untracked-files=no"]).is_some_and(|s| !s.is_empty());
    Some(if dirty {
        format!("{commit}-dirty")
    } else {
        commit
    })
}

/// Run `git -C dir args…`; its trimmed stdout when it succeeded.
pub fn git(dir: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    out.status.success().then(|| text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unstamped_build_reports_the_checkout_commit() {
        if option_env!("RUSTY_ENGINE_COMMIT").is_some() {
            return; // a release build reports its stamp instead
        }
        let checkout = git_commit(Path::new(env!("CARGO_MANIFEST_DIR")));
        assert_eq!(
            engine_commit(),
            checkout.as_deref().unwrap_or(UNKNOWN_COMMIT)
        );
    }

    /// A fresh repository with one commit of `tracked.txt`, under the temp dir.
    fn scratch_repo(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("rusty_{name}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("tracked.txt"), "one").unwrap();
        let commit = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qm",
            "c",
        ];
        for args in [&["init", "-q"][..], &["add", "tracked.txt"], &commit] {
            let ran = git(&dir, args);
            assert!(ran.is_some(), "git {args:?} failed in {}", dir.display());
        }
        dir
    }

    #[test]
    fn a_clean_checkout_reports_its_full_hash() {
        let dir = scratch_repo("git_commit_clean");
        let commit = git_commit(&dir).expect("a repository has a commit");
        assert_eq!(commit.len(), 40, "{commit}");
        assert!(commit.chars().all(|c| c.is_ascii_hexdigit()), "{commit}");
        std::fs::write(dir.join("untracked.txt"), "new").unwrap();
        assert_eq!(
            git_commit(&dir),
            Some(commit),
            "untracked files are not dirt"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_tracked_file_marks_the_commit_dirty() {
        let dir = scratch_repo("git_commit_dirty");
        let head = git(&dir, &["rev-parse", "HEAD"]).expect("a repository has a HEAD");
        std::fs::write(dir.join("tracked.txt"), "two").unwrap();
        assert_eq!(git_commit(&dir), Some(format!("{head}-dirty")));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn outside_a_repository_there_is_no_commit() {
        assert_eq!(git_commit(Path::new("/")), None);
    }
}
