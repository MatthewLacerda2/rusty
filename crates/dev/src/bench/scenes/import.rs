use std::path::{Path, PathBuf};

use rusty_asset::convert::IMPORT_CACHE_DIR;
use rusty_core::project::CACHE_DIR;

pub fn import() -> Result<f64, String> {
    let start = std::time::Instant::now();
    crate::export::import(Path::new(rusty_core::project::ASSETS_DIR))?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

pub fn pending(project: &Path) -> Result<Vec<PathBuf>, String> {
    rusty_core::project::open(project, rusty_core::project::Access::Run)?;
    let assets = Path::new(rusty_core::project::ASSETS_DIR);
    let mut found = rusty_asset::texture_import::pending(assets);
    found.extend(rusty_asset::cook::pending(assets));
    Ok(found)
}

const LEFT_OUT: [&str; 3] = [CACHE_DIR, IMPORT_CACHE_DIR, ".git"];

pub fn scratch(project: &Path, into: &Path) -> std::io::Result<()> {
    if into.exists() {
        std::fs::remove_dir_all(into)?;
    }
    std::fs::create_dir_all(into)?;
    let skip = std::fs::canonicalize(into)?;
    let mut copying = vec![std::fs::canonicalize(project)?];
    copy_tree(project, into, (Path::new(""), &skip), &mut copying)
}

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
        let (Ok(real), Ok(kind)) = (std::fs::canonicalize(&src), std::fs::metadata(&src)) else {
            continue;
        };
        // `skip` is the copy itself, when its folder sits inside the project.
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
