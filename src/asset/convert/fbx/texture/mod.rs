//! An FBX texture as a URI the written `.glb` can reference (#1082).
//!
//! A texture that is a file on disk is referenced where it lies. One embedded in
//! the FBX (`ufbx::Texture::content`: Mixamo and many store packs embed them) is
//! extracted into [`Staging::textures`] beside the `.glb`, the way Unity extracts
//! them on import, and referenced there. File names are sanitised and kept
//! distinct; what an earlier conversion left is `convert::refresh`'s to remove. Every
//! URI is percent-encoded (`gltf::uri`): the folder is named after the source file,
//! which may hold a `:` or a `%`.
//!
//! Every file on disk a texture is looked for at is reported as a [`Dependency`],
//! found or not, so the cache re-converts when one appears, vanishes or changes
//! (#1189). An embedded texture's file name is reserved even when a file on disk
//! is used instead, so a name in the textures folder holds the same bytes
//! whatever is on disk (`stage::move_in` relies on it).
//!
//! An extracted texture is named in the `.glb` by its [`identity`], which a saved
//! scene references it by (#1130); one on disk is not, and stays a plain path.
//!
//! A texture that is neither on disk nor embedded, is on disk but cannot be read
//! or loaded (not an image format the importer reads, bytes that disagree with
//! its name, or empty: #1293), or embeds a format it doesn't read, is a [`Skip`]:
//! the material then keeps its colour. A later file on disk, then an embedded
//! copy, stands in for one that can't be loaded. One that fails to extract, or
//! lies on disk where no URI can reach it, fails the conversion (#1168).

mod identity;

use super::super::texture_file::{self, file_name, relative_uri, sniff, unique};
use super::super::{glb, Dependency, Seen, Skip, Staging};
use crate::asset::gltf::uri;
use crate::asset::{is_image_path, ImportError, IMAGE_EXTENSIONS};
use crate::core::{collections::Map, paths};
use std::path::{Path, PathBuf};

/// Every texture one conversion references, by FBX element id.
pub struct Textures {
    /// Where the `.glb` will live: URIs are relative to it.
    out_dir: PathBuf,
    /// Where embedded textures are extracted.
    dir: PathBuf,
    by_id: Map<u32, Option<glb::Texture>>,
    /// Each texture's [`identity`], by element id.
    identities: Map<u32, String>,
    /// File names reserved in `dir` so far, extracted or not.
    written: Vec<String>,
    /// What was left out on purpose, in the order it was met.
    pub skipped: Vec<Skip>,
    /// Every file on disk looked at, in the order it was looked at.
    pub looked_at: Vec<Dependency>,
}

impl Textures {
    /// The textures of the `.glb` `to` stages, out of the scene's `textures`.
    pub fn new<'a>(to: &Staging, textures: impl IntoIterator<Item = &'a ufbx::Texture>) -> Self {
        Self {
            // `.` for a bare file name, as `stage` reads it: a URI is relative to
            // a folder that exists.
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

    /// `map`'s texture, extracting it on first use when it is embedded; `None`
    /// when it has none or it was skipped.
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
        // Reserved before the disk is looked at, so names never shift with it.
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

    /// The URI of `file`, a texture on disk, from the `.glb`'s folder.
    fn reference(&self, file: &Path) -> Result<String, ImportError> {
        relative_uri(&self.out_dir, file).ok_or_else(|| {
            ImportError::Parse(format!(
                "FBX texture {}: cannot reference it",
                file.display()
            ))
        })
    }

    /// The first file on disk `texture` names that the renderer can load, else why
    /// the first one there cannot be; each one looked at is recorded in
    /// [`Textures::looked_at`].
    fn on_disk(&mut self, texture: &ufbx::Texture) -> Result<Disk, ImportError> {
        let mut disk = Disk::Absent;
        for name in [&texture.filename, &texture.absolute_filename] {
            let path = PathBuf::from(name.as_ref() as &str);
            if path.as_os_str().is_empty() {
                continue;
            }
            // Read, not only stat'd: what was found is what the stamp records.
            let mut looked = Dependency::look(&path)?;
            let why = match looked.seen() {
                Seen::Read(_) => match usable(&path) {
                    Usable::Yes => {
                        self.looked_at.push(looked);
                        return Ok(Disk::Found(path));
                    }
                    Usable::No(why) => Some(why),
                    // A look cached by length and time can outlive the file's
                    // permissions: the stamp records what opening it found.
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

    /// Record `texture` as left out, for `why`: no URI.
    fn skip(&mut self, texture: &ufbx::Texture, why: String) -> Option<glb::Texture> {
        self.skipped.push(Skip {
            what: format!("FBX texture `{}`", texture.element.name),
            why,
        });
        None
    }
}

/// What looking on disk for a texture found.
enum Disk {
    Found(PathBuf),
    /// A file is there, but the renderer can't load it: why.
    Unusable(String),
    Absent,
}

impl Disk {
    /// Keep the first reason a file there can't be used.
    fn note(&mut self, why: Option<String>) {
        if let (Disk::Absent, Some(why)) = (&self, why) {
            *self = Disk::Unusable(why);
        }
    }
}

fn unreadable(file: &Path) -> String {
    format!("{} is on disk but cannot be read", file.display())
}

/// Whether the renderer can load `file`, which was read.
enum Usable {
    Yes,
    No(String),
    Unopened,
}

/// The renderer picks its decoder by the name's extension (#1293), so `file`'s
/// must be an image format it reads, and its bytes, when their magic number says
/// what they are, must agree. An empty file is no image. A `.psd`, `.tif` or
/// `.dds` beside an FBX is common in store packs.
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

/// The name `texture`'s file goes by: its file name, else its element name.
fn raw_name(texture: &ufbx::Texture) -> &str {
    [&texture.relative_filename, &texture.filename]
        .into_iter()
        .map(|name| name.as_ref() as &str)
        .find(|name| !name.is_empty())
        .unwrap_or(texture.element.name.as_ref())
}

#[cfg(test)]
mod tests;
