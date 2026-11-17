//! `import_ms`: a scene's load with the project's import cache cold, where
//! `load_ms` reads the cache as the project holds it (warm once the editor or
//! a run imported it).
//!
//! The project is copied once into a scratch folder ([`scratch`]) without its
//! caches: the machine-local `cache/` and the import cache
//! (`asset::convert::IMPORT_CACHE_DIR`, converted models and compressed
//! textures), and without `.git`. Before each scene the copy's caches are
//! cleared again ([`clear`]), and the scene is loaded there by the same timed
//! load as `load_ms` (`super::load`), whose Play refresh converts every model
//! source the project holds. The project itself is only read. Files are copied,
//! never hard-linked: a load that rewrites a file in place (a model's hull
//! sidecar) must not reach the original through a shared link.

use std::path::Path;

use rusty_asset::convert::IMPORT_CACHE_DIR;
use rusty_core::project::CACHE_DIR;

/// What the copy leaves out, relative to the project root.
const LEFT_OUT: [&str; 3] = [CACHE_DIR, IMPORT_CACHE_DIR, ".git"];

/// Copy the project at `project` into `into` (made afresh), without its caches.
/// A link is copied as a link, never followed.
pub fn scratch(project: &Path, into: &Path) -> std::io::Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    // A copy kept inside the project (an out dir under it) is not copied into itself.
    let skip = std::fs::canonicalize(into)?;
    copy_tree(project, into, (Path::new(""), &skip))
}

/// Remove the copy's caches, so the next load imports from nothing.
pub fn clear(scratch: &Path) -> std::io::Result<()> {
    for dir in [CACHE_DIR, IMPORT_CACHE_DIR] {
        let dir = scratch.join(dir);
        if dir.exists() {
            std::fs::remove_dir_all(dir)?;
        }
    }
    Ok(())
}

fn copy_tree(from: &Path, to: &Path, (rel, skip): (&Path, &Path)) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let rel = rel.join(entry.file_name());
        if LEFT_OUT.iter().any(|l| rel == Path::new(l)) {
            continue;
        }
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if src == skip {
            continue;
        }
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            link(&std::fs::read_link(&src)?, &dst)?;
        } else if kind.is_dir() {
            copy_tree(&src, &dst, (&rel, skip))?;
        } else {
            // lint: allow(asset-write) a scratch copy of the project, deleted after the bench
            std::fs::copy(&src, &dst)?;
        }
    }
    Ok(())
}

#[cfg(unix)]
fn link(target: &Path, at: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, at)
}

#[cfg(not(unix))]
fn link(_target: &Path, _at: &Path) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_copy_leaves_the_caches_out_and_clear_empties_them_again() {
        let root = rusty_test_support::temp::dir().join(format!("import_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let (project, copy) = (root.join("game"), root.join("copy"));
        for dir in ["assets/scenes", IMPORT_CACHE_DIR, CACHE_DIR, ".git"] {
            std::fs::create_dir_all(project.join(dir)).unwrap();
        }
        std::fs::write(project.join("assets/scenes/a.scene"), "{}").unwrap();
        std::fs::write(project.join(IMPORT_CACHE_DIR).join("m.glb"), "glb").unwrap();
        std::fs::write(project.join("cache/lock"), "").unwrap();
        scratch(&project, &copy).unwrap();
        assert!(copy.join("assets/scenes/a.scene").is_file());
        for left in LEFT_OUT {
            assert!(!copy.join(left).exists(), "{left} copied");
        }
        assert!(
            project.join(IMPORT_CACHE_DIR).join("m.glb").is_file(),
            "untouched"
        );
        std::fs::create_dir_all(copy.join(IMPORT_CACHE_DIR)).unwrap();
        clear(&copy).unwrap();
        assert!(!copy.join(IMPORT_CACHE_DIR).exists());
        assert!(copy.join("assets/scenes/a.scene").is_file());
    }
}
