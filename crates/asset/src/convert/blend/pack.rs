//! An image the conversion made is moved flat into [`Staging::textures`] named by its identity
//! (the image's name, made unique) and referenced as `model.blend#tex/<identity>`; one on disk
//! is referenced where it lies with its name dropped, since a name could only shadow a made
//! image's identity.
//! A made file whose bytes are no format the importer reads is a [`Skip`], dropped with what
//! samples it (`drop`).

use super::super::stage::hook::{self, Step};
use super::super::texture_file::{self, file_name, relative_uri, sniff, unique};
use super::super::{glb, relative_to_cwd, stage, Converted, Dependency, Skip, Staging};
use crate::gltf::uri;
use crate::{ImportError, IMAGE_EXTENSIONS};
use rusty_core::collections::Map;
use rusty_core::paths;
use serde::Deserialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

pub(super) const GLTF: &str = "model.gltf";
const REPORT: &str = "rusty.json";

#[derive(Deserialize)]
struct Report {
    libraries: Vec<PathBuf>,
    images: Vec<External>,
    made: Vec<Made>,
    skipped: Vec<Skipped>,
}

#[derive(Deserialize)]
struct Made {
    file: String,
    name: String,
}

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
    let made = report.made.into_iter().map(|m| (m.file, m.name)).collect();
    let mut images = Images::new(dir, to, made);
    images.resolve(&mut doc)?;
    super::drop::images(&mut doc, &images.dropped);
    converted.skipped.append(&mut images.skipped);
    let json = serde_json::to_vec(&doc).map_err(|e| failed(&gltf_path, &e.to_string()))?;
    hook::io(Step::WriteGltf)
        .and_then(|()| stage::write(&to.glb, &glb::container(json, bin)))
        .map_err(|e| failed(&to.glb, &e.to_string()))?;
    Ok(converted)
}

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

struct Images<'a> {
    dir: &'a Path,
    to: &'a Staging,
    /// The final `.glb`'s folder, not staging's: images on disk are referenced from it.
    out_dir: PathBuf,
    made: Map<String, String>,
    written: Vec<String>,
    dropped: Vec<usize>,
    skipped: Vec<Skip>,
}

impl<'a> Images<'a> {
    fn new(dir: &'a Path, to: &'a Staging, made: Map<String, String>) -> Self {
        let out_dir = match to.out.parent() {
            Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
            _ => PathBuf::from("."),
        };
        Self {
            dir,
            to,
            out_dir,
            made,
            written: Vec::new(),
            dropped: Vec::new(),
            skipped: Vec::new(),
        }
    }

    fn resolve(&mut self, doc: &mut Value) -> Result<(), ImportError> {
        let Some(images) = doc.get_mut("images").and_then(Value::as_array_mut) else {
            return Ok(());
        };
        for (index, image) in images.iter_mut().enumerate() {
            let Some(image) = image.as_object_mut() else {
                continue;
            };
            let Some(Value::String(old)) = image.get("uri") else {
                continue;
            };
            let decoded = uri::decode(old)
                .ok_or_else(|| failed(&self.dir.join(GLTF), &format!("a bad image URI `{old}`")))?;
            let file = paths::fold(&self.dir.join(decoded));
            let new = if file.starts_with(self.dir) {
                let Some((uri, identity)) = self.extract(&file)? else {
                    self.dropped.push(index);
                    continue;
                };
                image.insert("name".into(), Value::String(identity));
                uri
            } else {
                image.remove("name");
                relative_uri(&self.out_dir, &file)
                    .ok_or_else(|| failed(&file, "an image on disk the `.glb` cannot reference"))?
            };
            image.insert("uri".into(), Value::String(new));
        }
        Ok(())
    }

    fn extract(&mut self, file: &Path) -> Result<Option<(String, String)>, ImportError> {
        let bytes = read(file)?;
        let made = file
            .file_name()
            .map_or_else(Default::default, |n| n.to_string_lossy());
        let identity =
            self.made.get(made.as_ref()).cloned().ok_or_else(|| {
                failed(file, "an image the export script made but gave no identity")
            })?;
        let ext = made.rsplit_once('.').map_or("", |(_, ext)| ext);
        if !sniff(&bytes).is_some_and(|kind| IMAGE_EXTENSIONS.contains(&kind)) {
            self.skipped.push(Skip {
                what: format!("Blender image `{identity}`"),
                why: "its bytes are not an image rusty reads (PNG, JPEG)".into(),
            });
            return Ok(None);
        }
        let (stem, ext) = file_name(&format!("{identity}.{ext}"), &bytes);
        let name = unique(&stem, &ext, &self.written);
        self.written.push(name.clone());
        texture_file::write(&self.to.textures, &name, &bytes)?;
        let folder =
            (self.to.textures.file_name()).map_or_else(Default::default, |n| n.to_string_lossy());
        Ok(Some((
            uri::encode_path(&format!("{folder}/{name}")),
            identity,
        )))
    }
}

/// `path` relative to the working directory when it lies in it, so a stamp holds on every clone
/// of the project.
fn local(path: &Path) -> PathBuf {
    match relative_to_cwd(path) {
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
