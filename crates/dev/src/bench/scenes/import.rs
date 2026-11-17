//! `import_ms`: a scene's load with the project's import cache cold, where
//! `load_ms` reads the cache as the project holds it (warm once the editor or
//! a run imported it).
//!
//! Asked for (`IMPORT=1`), once a run: the project is copied into a scratch
//! folder ([`scratch`]) without its caches: the machine-local `cache/` and the
//! import cache (`asset::convert::IMPORT_CACHE_DIR`, converted models and
//! compressed textures), and without `.git`. The run's first scene is loaded
//! there by the same timed load as `load_ms` (`super::load`), whose Play refresh
//! converts every model source the project holds, so the figure is the
//! project's, not that scene's. The project itself is only read. Files are copied,
//! never hard-linked: a load that rewrites a file in place (a model's hull
//! sidecar) must not reach the original through a shared link.

use std::path::{Path, PathBuf};

use rusty_asset::convert::IMPORT_CACHE_DIR;
use rusty_core::project::CACHE_DIR;

/// What the copy leaves out, relative to the project root.
const LEFT_OUT: [&str; 3] = [CACHE_DIR, IMPORT_CACHE_DIR, ".git"];

/// Copy the project at `project` into `into` (made afresh), without its caches.
/// A link is followed and what it points to copied (a file's contents, a
/// folder's tree), so the copy stands alone: a relative link out of the project
/// still resolves, and no load in the copy writes through one into the original.
/// A link back into a folder being copied (a loop) is left out, and so is what
/// is neither a file nor a folder (a FIFO would block the copy for good).
pub fn scratch(project: &Path, into: &Path) -> std::io::Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    // A copy kept inside the project (an out dir under it) is not copied into itself.
    let skip = std::fs::canonicalize(into)?;
    let mut copying = vec![std::fs::canonicalize(project)?];
    copy_tree(project, into, (Path::new(""), &skip), &mut copying)
}

/// Copy `from` into `to`; `copying` holds the canonical folders being copied,
/// from the root down, which a link back into one would loop through.
fn copy_tree(
    from: &Path,
    to: &Path,
    (rel, skip): (&Path, &Path),
    copying: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let rel = rel.join(entry.file_name());
        if LEFT_OUT.iter().any(|l| rel == Path::new(l)) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        // A dangling link has nothing to copy.
        let (Ok(real), Ok(kind)) = (std::fs::canonicalize(&src), std::fs::metadata(&src)) else {
            continue;
        };
        if real == skip || copying.contains(&real) {
            continue;
        }
        if kind.is_dir() {
            copying.push(real);
            copy_tree(&src, &dst, (&rel, skip), copying)?;
            copying.pop();
        } else if kind.is_file() {
            // lint: allow(asset-write) a scratch copy of the project, deleted after the bench
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "import_tests.rs"]
mod tests;
