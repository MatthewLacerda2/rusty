use crate::repo::ls_files;
use std::path::{Path, PathBuf};

pub(super) fn committable(base: &Path, roots: &[&str]) -> Result<Vec<PathBuf>, String> {
    // `--cached` still lists a tracked file deleted from the working tree.
    let flags = ["--cached", "--others", "--exclude-standard"];
    Ok(ls_files(base, &flags, roots)?
        .into_iter()
        .map(|p| base.join(p))
        .filter(|p| p.is_file())
        .collect())
}

#[cfg(test)]
#[path = "listing_tests.rs"]
mod tests;
