mod copy;
mod textures;

use std::path::{Path, PathBuf};

use rusty_core::paths;
use rusty_core::project::{create_skeleton, is_project, ASSETS_DIR, PROJECT_FILE, SHADERS_DIR};
use rusty_edit::io::SEED_MANIFEST_PATH;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Exported {
    pub files: Vec<String>,
    pub sources: Vec<PathBuf>,
}

pub fn export(out: &Path) -> Result<Exported, String> {
    let out = std::path::absolute(out).map_err(|e| format!("{}: {e}", out.display()))?;
    check(&rusty_core::project::root(), &out)?;
    seed_and_refresh_audio()?;
    let sources = import(Path::new(ASSETS_DIR))?.sources;
    rusty_shadergen::rebake::refresh_all(Path::new(SHADERS_DIR));
    let existed = out.is_dir();
    let written = copy_runtime_files(Path::new("."), &sources, &out).and_then(|files| {
        create_skeleton(&out)?;
        Ok(files)
    });
    match written {
        Ok(files) => Ok(Exported { files, sources }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&out);
            if existed {
                let _ = std::fs::create_dir(&out);
            }
            Err(e)
        }
    }
}

pub fn import(root: &Path) -> Result<rusty_asset::import::Imported, String> {
    rusty_asset::import::import_strict(root)
}

fn seed_and_refresh_audio() -> Result<(), String> {
    let refresh = rusty_edit::seed_project();
    let converted = refresh.lines().into_iter().filter(|(warning, _)| !warning);
    for (_, line) in converted {
        println!("{line}");
    }
    let mut failed = Vec::new();
    for (mp3, why) in &refresh.skipped {
        let wav = mp3.with_extension("wav");
        match wav.is_file() {
            true => eprintln!(
                "[Export] {} is not shipped: {} already exists and ships in its place \
                 (the editor plays the MP3, the game the WAV)",
                mp3.display(),
                wav.display()
            ),
            false => failed.push(format!("{}: {why}", mp3.display())),
        }
    }
    match failed.is_empty() {
        true => Ok(()),
        false => Err(format!(
            "MP3s that could not be converted to WAV (no MP3 ships):\n  {}",
            failed.join("\n  ")
        )),
    }
}

pub fn check(project: &Path, out: &Path) -> Result<(), String> {
    if !is_project(project) {
        return Err(format!(
            "{}: not a rusty project (no {PROJECT_FILE} and no {ASSETS_DIR}/)",
            project.display()
        ));
    }
    check_out(project, out)
}

fn copy_runtime_files(root: &Path, sources: &[PathBuf], out: &Path) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    for file in [PROJECT_FILE, SEED_MANIFEST_PATH] {
        if root.join(file).is_file() {
            copy::file(root, Path::new(file), out, &mut files)?;
        }
    }
    copy::tree(root, Path::new(ASSETS_DIR), out, &mut files)?;
    let glbs = copy::caches(root, sources, out, &mut files)?;
    let images = rusty_asset::texture_import::images(&root.join(ASSETS_DIR));
    let images: Vec<PathBuf> = images
        .iter()
        .map(|i| i.strip_prefix(root).unwrap_or(i).to_path_buf())
        .collect();
    copy::texture_caches(root, &images, out, &mut files)?;
    copy::model_caches(root, out, &mut files)?;
    let missing = textures::missing(out, &glbs, &files);
    if !missing.is_empty() {
        return Err(format!(
            "textures outside the exported folders (move them under {ASSETS_DIR}/):\n  {}",
            missing.join("\n  ")
        ));
    }
    Ok(files)
}

fn check_out(root: &Path, out: &Path) -> Result<(), String> {
    if out.exists() && !out.is_dir() {
        return Err(format!(
            "{}: a file; export into a new or empty folder",
            out.display()
        ));
    }
    let (root, resolved) = (resolved(root), resolved(out));
    if resolved.starts_with(&root) || root.starts_with(&resolved) {
        return Err(format!(
            "{}: the output must lie outside the project {}",
            out.display(),
            root.display()
        ));
    }
    let occupied = std::fs::read_dir(out).is_ok_and(|mut entries| entries.next().is_some());
    if occupied {
        return Err(format!(
            "{}: not empty; export into a new or empty folder",
            out.display()
        ));
    }
    Ok(())
}

fn resolved(path: &Path) -> PathBuf {
    let path = paths::fold(&std::path::absolute(path).unwrap_or(path.to_path_buf()));
    for ancestor in path.ancestors() {
        if let Ok(canonical) = std::fs::canonicalize(ancestor) {
            let rest = path.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return canonical.join(rest);
        }
    }
    path
}

#[cfg(test)]
mod tests;
