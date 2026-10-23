//! What Blender exported, made into the cached `.glb` (#1072).
//!
//! The export script writes a glTF with its buffer and the images the conversion
//! made beside it (`export.py`). This joins the JSON and the buffer into one
//! binary glTF (`glb::container`), and resolves each image:
//!
//! - one the conversion made (a packed or generated image, under the export
//!   folder) is moved into [`Staging::textures`], flat, and keeps its name: the
//!   importer references it as `model.blend#tex/<name>` (#1130);
//! - one that is a file on disk is referenced where it lies, from the `.glb`'s
//!   folder, and the importer reads it as the plain path it is.
//!
//! Every file the `.blend` named (a linked library, an image on disk) is looked at
//! and recorded, found or not, so the cache re-converts when one changes (#1189).

use super::super::stage::hook::{self, Step};
use super::super::texture_file::{file_name, relative_uri, unique};
use super::super::{glb, stage, Converted, Dependency, Skip, Staging};
use crate::asset::gltf::uri;
use crate::asset::ImportError;
use crate::core::paths;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// The glTF the export script writes, in the export folder.
pub(super) const GLTF: &str = "model.gltf";
/// What the export script looked at and left out, beside it.
const REPORT: &str = "rusty.json";

/// `rusty.json`: what the export script found in the `.blend`.
#[derive(Deserialize)]
struct Report {
    libraries: Vec<PathBuf>,
    images: Vec<External>,
    skipped: Vec<Skipped>,
}

/// An image the `.blend` names as a file on disk.
#[derive(Deserialize)]
struct External {
    name: String,
    path: PathBuf,
}

#[derive(Deserialize)]
struct Skipped {
    what: String,
    why: String,
}

/// Turn the export in `dir` into the `.glb` and textures `to` stages.
pub(super) fn pack(dir: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let report: Report = serde_json::from_slice(&read(&dir.join(REPORT))?)
        .map_err(|e| failed(&dir.join(REPORT), &e.to_string()))?;
    let mut converted = Converted {
        skipped: (report.skipped.into_iter())
            .map(|s| Skip {
                what: s.what,
                why: s.why,
            })
            .collect(),
        depends_on: Vec::new(),
    };
    for library in &report.libraries {
        converted
            .depends_on
            .push(Dependency::look(&local(library))?);
    }
    for image in &report.images {
        let looked = Dependency::look(&local(&image.path))?;
        if !looked.found() {
            converted.skipped.push(Skip {
                what: format!("Blender image `{}`", image.name),
                why: format!("{} is not a file that can be read", image.path.display()),
            });
        }
        converted.depends_on.push(looked);
    }
    let gltf_path = dir.join(GLTF);
    let mut doc: Value = serde_json::from_slice(&read(&gltf_path)?)
        .map_err(|e| failed(&gltf_path, &e.to_string()))?;
    let bin = buffer(dir, &mut doc)?;
    Images::new(dir, to).resolve(&mut doc)?;
    let json = serde_json::to_vec(&doc).map_err(|e| failed(&gltf_path, &e.to_string()))?;
    hook::io(Step::WriteGltf)
        .and_then(|()| stage::write(&to.glb, &glb::container(json, bin)))
        .map_err(|e| failed(&to.glb, &e.to_string()))?;
    Ok(converted)
}

/// The bytes of the export's one buffer, its `uri` removed so it becomes the
/// `.glb`'s binary chunk. No buffer is an empty one.
fn buffer(dir: &Path, doc: &mut Value) -> Result<Vec<u8>, ImportError> {
    let Some(buffers) = doc.get_mut("buffers").and_then(Value::as_array_mut) else {
        return Ok(Vec::new());
    };
    let [buffer] = buffers.as_mut_slice() else {
        let why = format!("{} buffers, where one is expected", buffers.len());
        return Err(failed(&dir.join(GLTF), &why));
    };
    let Some(Value::String(name)) = buffer.as_object_mut().and_then(|b| b.remove("uri")) else {
        return Err(failed(&dir.join(GLTF), "its buffer names no file"));
    };
    let decoded = uri::decode(&name).ok_or_else(|| failed(&dir.join(GLTF), "a bad buffer URI"))?;
    read(&dir.join(decoded))
}

/// The images of one export, as they are resolved.
struct Images<'a> {
    /// The export folder: an image under it is one the conversion made.
    dir: &'a Path,
    to: &'a Staging,
    /// The `.glb`'s final folder, which an image on disk is referenced from.
    out_dir: PathBuf,
    /// File names written into the textures folder so far.
    written: Vec<String>,
}

impl<'a> Images<'a> {
    fn new(dir: &'a Path, to: &'a Staging) -> Self {
        let out_dir = match to.out.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        Self {
            dir,
            to,
            out_dir,
            written: Vec::new(),
        }
    }

    /// Point every image with a URI at where it lies once swapped in.
    fn resolve(&mut self, doc: &mut Value) -> Result<(), ImportError> {
        let Some(images) = doc.get_mut("images").and_then(Value::as_array_mut) else {
            return Ok(());
        };
        for image in images.iter_mut().filter_map(Value::as_object_mut) {
            let Some(Value::String(old)) = image.get("uri") else {
                continue;
            };
            let decoded = uri::decode(old)
                .ok_or_else(|| failed(&self.dir.join(GLTF), &format!("a bad image URI `{old}`")))?;
            let file = paths::fold(&self.dir.join(decoded));
            let new = if file.starts_with(self.dir) {
                self.extract(&file)?
            } else {
                // Named still: the importer makes it a plain path, as it lies
                // outside the cache (`gltf::material`).
                relative_uri(&self.out_dir, &file)
                    .ok_or_else(|| failed(&file, "an image on disk the `.glb` cannot reference"))?
            };
            image.insert("uri".into(), Value::String(new));
        }
        Ok(())
    }

    /// Move `file`, an image the conversion made, into the textures folder: its URI
    /// there. Only the textures folder is created, never the staging folder above
    /// it (see `fbx::texture`'s `extract`).
    fn extract(&mut self, file: &Path) -> Result<String, ImportError> {
        let bytes = read(file)?;
        let raw = file
            .file_name()
            .map_or_else(Default::default, |n| n.to_string_lossy());
        let (stem, ext) = file_name(&raw, &bytes);
        let name = unique(&stem, &ext, &self.written);
        self.written.push(name.clone());
        let dir = &self.to.textures;
        match std::fs::create_dir(dir) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                return Err(failed(dir, &e.to_string()))
            }
            _ => {}
        }
        let path = dir.join(&name);
        hook::io(Step::ExtractTexture)
            .and_then(|()| stage::write(&path, &bytes))
            .map_err(|e| failed(&path, &e.to_string()))?;
        let folder = dir
            .file_name()
            .map_or_else(Default::default, |n| n.to_string_lossy());
        Ok(uri::encode_path(&format!("{folder}/{name}")))
    }
}

/// `path` relative to the working directory when it lies in it, as a project's
/// paths are written: a stamp that holds on every clone of the project.
fn local(path: &Path) -> PathBuf {
    let cwd = std::env::current_dir().ok();
    let cwd = cwd.and_then(|c| paths::canonicalize(&c).ok());
    match cwd.and_then(|c| path.strip_prefix(c).ok().map(Path::to_path_buf)) {
        Some(rel) if !rel.as_os_str().is_empty() => rel,
        _ => path.to_path_buf(),
    }
}

fn read(path: &Path) -> Result<Vec<u8>, ImportError> {
    std::fs::read(path).map_err(|e| failed(path, &e.to_string()))
}

fn failed(path: &Path, why: &str) -> ImportError {
    ImportError::Parse(format!("{}: {why}", path.display()))
}
