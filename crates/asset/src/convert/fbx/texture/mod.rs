//! An embedded name is reserved even when a disk file is used, so the textures
//! folder holds the same bytes whatever is on disk (`stage::move_in` relies on it).
//! An unusable texture is a [`Skip`]; one that fails to extract, or that no URI reaches, fails
//! the conversion.

mod identity;

use super::super::texture_file::{self, file_name, relative_uri, sniff, unique};
use super::super::{glb, Dependency, Seen, Skip, Staging};
use crate::gltf::uri;
use crate::{is_image_path, ImportError, IMAGE_EXTENSIONS};
use rusty_core::{collections::Map, paths};
use std::path::{Path, PathBuf};

pub struct Textures {
    out_dir: PathBuf,
    dir: PathBuf,
    by_id: Map<u32, Option<glb::Texture>>,
    identities: Map<u32, String>,
    written: Vec<String>,
    pub skipped: Vec<Skip>,
    pub looked_at: Vec<Dependency>,
}

impl Textures {
    pub fn new<'a>(to: &Staging, textures: impl IntoIterator<Item = &'a ufbx::Texture>) -> Self {
        Self {
            out_dir: match to.out.parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
                _ => PathBuf::from("."),
            },
            dir: to.textures.clone(),
            by_id: Map::default(),
            identities: identity::identities(textures),
            written: Vec::new(),
            skipped: Vec::new(),
            looked_at: Vec::new(),
        }
    }

    pub fn texture(
        &mut self,
        map: &ufbx::MaterialMap,
    ) -> Result<Option<glb::Texture>, ImportError> {
        let Some(texture) = map.texture.as_ref() else {
            return Ok(None);
        };
        let id = texture.element.element_id;
        if let Some(known) = self.by_id.get(&id) {
            return Ok(known.clone());
        }
        let resolved = self.resolve(texture)?;
        self.by_id.insert(id, resolved.clone());
        Ok(resolved)
    }

    fn resolve(&mut self, texture: &ufbx::Texture) -> Result<Option<glb::Texture>, ImportError> {
        // Reserved before the disk is looked at: names never shift with it.
        let embedded = (!texture.content.is_empty())
            .then(|| file_name(raw_name(texture), &texture.content))
            .map(|(stem, ext)| {
                let readable = IMAGE_EXTENSIONS.contains(&ext.as_str());
                let name = readable.then(|| unique(&stem, &ext, &self.written));
                self.written.extend(name.clone());
                (name, ext)
            });
        let unusable = match self.on_disk(texture)? {
            Disk::Found(file) => {
                let uri = self.reference(&file)?;
                return Ok(Some(glb::Texture { uri, name: None }));
            }
            Disk::Unusable(why) => Some(why),
            Disk::Absent => None,
        };
        let name = match (embedded, unusable) {
            (Some((Some(name), _)), _) => name,
            (Some((None, ext)), _) => {
                let why = format!("embeds a `.{ext}`, not an image format the importer reads");
                return Ok(self.skip(texture, why));
            }
            (None, Some(why)) => return Ok(self.skip(texture, why)),
            (None, None) => {
                let why = "neither a file on disk nor embedded".into();
                return Ok(self.skip(texture, why));
            }
        };
        texture_file::write(&self.dir, &name, &texture.content)?;
        let Some(dir) = self.dir.file_name() else {
            return Err(ImportError::Parse(format!(
                "FBX texture `{}`: {} names no folder to reference",
                texture.element.name,
                self.dir.display()
            )));
        };
        Ok(Some(glb::Texture {
            uri: uri::encode_path(&format!("{}/{name}", dir.to_string_lossy())),
            name: self.identities.get(&texture.element.element_id).cloned(),
        }))
    }

    fn reference(&self, file: &Path) -> Result<String, ImportError> {
        relative_uri(&self.out_dir, file).ok_or_else(|| {
            ImportError::Parse(format!(
                "FBX texture {}: cannot reference it",
                file.display()
            ))
        })
    }

    fn on_disk(&mut self, texture: &ufbx::Texture) -> Result<Disk, ImportError> {
        let mut disk = Disk::Absent;
        for name in [&texture.filename, &texture.absolute_filename] {
            let path = PathBuf::from(name.as_ref() as &str);
            if path.as_os_str().is_empty() {
                continue;
            }
            let mut looked = Dependency::look(&path)?;
            let why = match looked.seen() {
                Seen::Read(_) => match usable(&path) {
                    Usable::Yes => {
                        self.looked_at.push(looked);
                        return Ok(Disk::Found(path));
                    }
                    Usable::No(why) => Some(why),
                    // A cached look can outlive the file's permissions: the stamp records what
                    // opening it found.
                    Usable::Unopened => {
                        looked = Dependency::unopened(&path);
                        Some(unreadable(&path))
                    }
                },
                Seen::Unreadable => Some(unreadable(&path)),
                Seen::Absent => None,
            };
            self.looked_at.push(looked);
            disk.note(why);
        }
        Ok(disk)
    }

    fn skip(&mut self, texture: &ufbx::Texture, why: String) -> Option<glb::Texture> {
        self.skipped.push(Skip {
            what: format!("FBX texture `{}`", texture.element.name),
            why,
        });
        None
    }
}

enum Disk {
    Found(PathBuf),
    Unusable(String),
    Absent,
}

impl Disk {
    fn note(&mut self, why: Option<String>) {
        if let (Disk::Absent, Some(why)) = (&self, why) {
            *self = Disk::Unusable(why);
        }
    }
}

fn unreadable(file: &Path) -> String {
    format!("{} is on disk but cannot be read", file.display())
}

enum Usable {
    Yes,
    No(String),
    Unopened,
}

/// The renderer decodes by extension, so the extension must be an image format and
/// the bytes' magic number must agree with it.
fn usable(file: &Path) -> Usable {
    use std::io::Read;
    let shown = file.display();
    if !is_image_path(file) {
        return Usable::No(format!("{shown} is not an image format the importer reads"));
    }
    let mut head = [0u8; 8];
    let Ok(read) = std::fs::File::open(file).and_then(|mut f| f.read(&mut head)) else {
        return Usable::Unopened;
    };
    let ext = paths::extension(file);
    match sniff(&head[..read]) {
        _ if read == 0 => Usable::No(format!("{shown} is empty")),
        Some("jpg") if ext == "jpeg" => Usable::Yes,
        Some(sniffed) if sniffed != ext => Usable::No(format!(
            "{shown} holds `.{sniffed}` bytes under a `.{ext}` name"
        )),
        _ => Usable::Yes,
    }
}

fn raw_name(texture: &ufbx::Texture) -> &str {
    [&texture.relative_filename, &texture.filename]
        .into_iter()
        .map(|name| name.as_ref() as &str)
        .find(|name| !name.is_empty())
        .unwrap_or(texture.element.name.as_ref())
}

#[cfg(test)]
mod tests;
