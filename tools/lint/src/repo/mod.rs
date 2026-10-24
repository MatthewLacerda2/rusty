//! The repository as git sees it: the one place the lint tool asks git which files
//! exist (#1387). The size gate lists what can be committed and the `sorted` rule
//! lists the tracked Markdown; both go through [`ls_files`], so they decode names and
//! report git's failures the same way.

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
pub mod scaffold;
#[cfg(test)]
mod tests;

/// `git -C root`, answering for the repository at `root` and never for one the
/// caller's environment names: under a hook or `git rebase -x`, `GIT_DIR` and its
/// kin point at the outer repository.
pub fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    for var in ["GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"] {
        cmd.env_remove(var);
    }
    // A scratch repo under the temp dir must not discover a repo above it.
    #[cfg(test)]
    if let Some(tmp) = root
        .starts_with(std::env::temp_dir())
        .then(|| root.parent())
    {
        cmd.env("GIT_CEILING_DIRECTORIES", tmp.unwrap_or(root));
    }
    cmd.arg("-C").arg(if root.as_os_str().is_empty() {
        Path::new(".")
    } else {
        root
    });
    cmd
}

/// Git's file names are raw bytes: on Unix they are kept as such, so a name that
/// isn't UTF-8 still names the file on disk.
fn path_of(bytes: &[u8]) -> PathBuf {
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
    }
    #[cfg(not(unix))]
    {
        PathBuf::from(String::from_utf8_lossy(bytes).into_owned())
    }
}

/// `git ls-files -z <flags> -- <pathspecs>` in the repository at `root`: the paths
/// relative to it, sorted and deduplicated (a file with a merge conflict is listed
/// once per stage). `Err` carries git's own message, or why it couldn't run.
pub fn ls_files(root: &Path, flags: &[&str], pathspecs: &[&str]) -> Result<Vec<PathBuf>, String> {
    let out = git(root)
        .args(["ls-files", "-z"])
        .args(flags)
        .arg("--")
        .args(pathspecs)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("git {}: {why}", out.status));
    }
    let mut files: Vec<PathBuf> = out
        .stdout
        .split(|&b| b == 0)
        .filter(|p| !p.is_empty())
        .map(path_of)
        .collect();
    files.sort();
    files.dedup();
    Ok(files)
}
