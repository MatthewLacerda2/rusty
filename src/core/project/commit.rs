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
