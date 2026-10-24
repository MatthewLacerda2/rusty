//! src/core/project/engine/commit.rs — which engine commit this binary is, and where
//! it sits in history (#853, #931, #1319).
//!
//! A **release** build has both stamped in at compile time (`build.rs` sets
//! `RUSTY_ENGINE_COMMIT` and `RUSTY_ENGINE_ORDER`), because a shipped binary has no
//! checkout to ask. A **debug** build asks git at run time, in the checkout it was
//! built from: stamping it there would change the crate's fingerprint on every
//! commit, checkout and rebase, and rebuild the whole crate once per feature set
//! (#931 measured ~15 s each).
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
    #[cfg(test)]
    if let Some((commit, _)) = PINNED.get() {
        return commit;
    }
    static COMMIT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    COMMIT.get_or_init(|| match option_env!("RUSTY_ENGINE_COMMIT") {
        Some(stamped) => stamped.to_string(),
        None => git_commit(Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|| UNKNOWN_COMMIT.to_string()),
    })
}

/// This engine's place in history: how many commits lead to it (`git rev-list
/// --count HEAD`). `main` is squash-merged and linear, so a later `main` engine
/// always counts higher. `None` when it can't be counted: no git, or a shallow
/// clone, whose count is only its depth.
pub fn engine_order() -> Option<u64> {
    #[cfg(test)]
    if let Some((_, order)) = PINNED.get() {
        return order;
    }
    static ORDER: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *ORDER.get_or_init(|| match option_env!("RUSTY_ENGINE_COMMIT") {
        Some(_) => option_env!("RUSTY_ENGINE_ORDER").and_then(|n| n.parse().ok()),
        None => git_order(Path::new(env!("CARGO_MANIFEST_DIR"))),
    })
}

/// `git rev-list --count HEAD` in `dir`; `None` without git, outside a repository
/// or in a shallow clone (an old git that can't say whether it is one counts as one).
pub fn git_order(dir: &Path) -> Option<u64> {
    let shallow = git(dir, &["rev-parse", "--is-shallow-repository"])?;
    if shallow != "false" {
        return None;
    }
    git(dir, &["rev-list", "--count", "HEAD"])?.parse().ok()
}

#[cfg(test)]
thread_local! {
    static PINNED: std::cell::Cell<Option<(&'static str, Option<u64>)>> =
        const { std::cell::Cell::new(None) };
}

/// Make this test thread's running engine `commit` at `order`, so a test of the
/// engine check means the same in a full clone, a shallow one and without git.
#[cfg(test)]
pub fn pin(commit: &'static str, order: Option<u64>) {
    PINNED.set(Some((commit, order)));
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
    fn an_unstamped_build_reports_the_checkout_commit_and_order() {
        if option_env!("RUSTY_ENGINE_COMMIT").is_some() {
            return; // a release build reports its stamp instead
        }
        let checkout = Path::new(env!("CARGO_MANIFEST_DIR"));
        let commit = git_commit(checkout);
        assert_eq!(engine_commit(), commit.as_deref().unwrap_or(UNKNOWN_COMMIT));
        assert_eq!(engine_order(), git_order(checkout));
    }

    #[test]
    fn a_pinned_engine_is_this_threads_running_one() {
        pin("abc", Some(7));
        assert_eq!((engine_commit(), engine_order()), ("abc", Some(7)));
    }

    /// A fresh repository with one commit of `tracked.txt`, under the test's temp dir.
    fn scratch_repo(name: &str) -> std::path::PathBuf {
        let dir = crate::test_temp::dir().join(name);
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
    fn a_full_clone_counts_its_commits_and_a_shallow_one_does_not() {
        let dir = scratch_repo("git_order_full");
        assert_eq!(git_order(&dir), Some(1));
        let shallow = crate::test_temp::dir().join("git_order_shallow");
        let _ = std::fs::remove_dir_all(&shallow);
        std::fs::write(dir.join("tracked.txt"), "two").unwrap();
        let commit = [
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "commit",
            "-qam",
            "d",
        ];
        assert!(git(&dir, &commit).is_some());
        assert_eq!(git_order(&dir), Some(2));
        let url = format!("file://{}", dir.display());
        let clone = ["clone", "-q", "--depth", "1", &url, "git_order_shallow"];
        assert!(git(&crate::test_temp::dir(), &clone).is_some());
        assert_eq!(git_order(&shallow), None, "a depth of 1 is not a count");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&shallow);
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
        assert_eq!(git_order(Path::new("/")), None);
    }
}
