use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(test)]
pub mod scaffold;
#[cfg(test)]
mod tests;

pub fn git(root: &Path) -> Command {
    let mut cmd = Command::new("git");
    cmd.env("LC_ALL", "C");
    for var in ["GIT_DIR", "GIT_INDEX_FILE", "GIT_WORK_TREE"] {
        cmd.env_remove(var);
    }
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

fn path_of(bytes: &[u8]) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
}

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
