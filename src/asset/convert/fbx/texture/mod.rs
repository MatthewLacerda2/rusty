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
//! A texture that is neither on disk nor embedded, or embeds a format the
//! importer does not read, is a [`Skip`]; one that fails to extract, or lies on
//! disk where no URI can reach it, fails the conversion (#1168).

use super::super::stage::hook::{self, Step};
use super::super::{Skip, Staging};
use crate::asset::gltf::uri;
use crate::asset::{ImportError, IMAGE_EXTENSIONS};
use crate::core::collections::Map;
use crate::core::paths;
use std::path::{Component, Path, PathBuf};

/// Every texture one conversion references, by FBX element id.
pub struct Textures {
    /// Where the `.glb` will live: URIs are relative to it.
    out_dir: PathBuf,
    /// Where embedded textures are extracted.
    dir: PathBuf,
    by_id: Map<u32, Option<String>>,
    /// File names extracted into `dir` so far.
    written: Vec<String>,
    /// What was left out on purpose, in the order it was met.
    pub skipped: Vec<Skip>,
}

impl Textures {
    /// The textures of the `.glb` `to` stages.
    pub fn new(to: &Staging) -> Self {
        Self {
            // `.` for a bare file name, as `stage` reads it: a URI is relative to
            // a folder that exists.
            out_dir: match to.out.parent() {
                Some(dir) if !dir.as_os_str().is_empty() => dir.to_path_buf(),
                _ => PathBuf::from("."),
            },
            dir: to.textures.clone(),
            by_id: Map::default(),
            written: Vec::new(),
            skipped: Vec::new(),
        }
    }

    /// The URI of `map`'s texture, extracting it on first use when it is embedded;
    /// `None` when it has none or it was skipped.
    pub fn uri(&mut self, map: &ufbx::MaterialMap) -> Result<Option<String>, ImportError> {
        let Some(texture) = map.texture.as_ref() else {
            return Ok(None);
        };
        let id = texture.element.element_id;
        if let Some(uri) = self.by_id.get(&id) {
            return Ok(uri.clone());
        }
        let uri = self.resolve(texture)?;
        self.by_id.insert(id, uri.clone());
        Ok(uri)
    }

    fn resolve(&mut self, texture: &ufbx::Texture) -> Result<Option<String>, ImportError> {
        let on_disk = [&texture.filename, &texture.absolute_filename]
            .into_iter()
            .map(|name| PathBuf::from(name.as_ref() as &str))
            .find(|path| !path.as_os_str().is_empty() && path.is_file());
        if let Some(file) = on_disk {
            return relative_uri(&self.out_dir, &file).map(Some).ok_or_else(|| {
                ImportError::Parse(format!(
                    "FBX texture {}: cannot reference it",
                    file.display()
                ))
            });
        }
        if texture.content.is_empty() {
            return Ok(self.skip(texture, "neither a file on disk nor embedded".into()));
        }
        let (stem, ext) = file_name(raw_name(texture), &texture.content);
        if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            let why = format!("embeds a `.{ext}`, not an image format the importer reads");
            return Ok(self.skip(texture, why));
        }
        let name = unique(&stem, &ext, &self.written);
        self.extract(&name, &texture.content)?;
        let Some(dir) = self.dir.file_name() else {
            return Err(ImportError::Parse(format!(
                "FBX texture `{}`: {} names no folder to reference",
                texture.element.name,
                self.dir.display()
            )));
        };
        let uri = uri::encode_path(&format!("{}/{name}", dir.to_string_lossy()));
        self.written.push(name);
        Ok(Some(uri))
    }

    /// Record `texture` as left out, for `why`: no URI.
    fn skip(&mut self, texture: &ufbx::Texture, why: String) -> Option<String> {
        self.skipped.push(Skip {
            what: format!("FBX texture `{}`", texture.element.name),
            why,
        });
        None
    }

    /// Write one texture. Only the textures folder is created, never the staging
    /// folder above it: a concurrent conversion that cleared that one must make
    /// this conversion fail, not resurrect half of it.
    fn extract(&self, name: &str, bytes: &[u8]) -> Result<(), ImportError> {
        match std::fs::create_dir(&self.dir) {
            Err(e) if e.kind() != std::io::ErrorKind::AlreadyExists => {
                return Err(ImportError::Parse(format!(
                    "writing {}: {e}",
                    self.dir.display()
                )))
            }
            _ => {}
        }
        let path = self.dir.join(name);
        hook::io(Step::ExtractTexture)
            .and_then(|()| std::fs::write(&path, bytes))
            .map_err(|e| ImportError::Parse(format!("writing {}: {e}", path.display())))
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

/// The sanitised stem and lowercase extension of the file `raw` names, holding
/// `bytes`: only ASCII letters, digits, `-`, `_` and inner `.`, so no separator,
/// `..` or URI escape survives. A name without an image extension whose bytes are
/// a PNG or a JPEG gets that extension (`Material #25` → `Material__25.png`).
fn file_name(raw: &str, bytes: &[u8]) -> (String, String) {
    // FBX paths are often Windows ones: split on both separators.
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let (stem, ext) = base.rsplit_once('.').unwrap_or((base, ""));
    let ext = clean(ext).to_ascii_lowercase();
    let (stem, ext) = match sniff(bytes) {
        Some(sniffed) if !IMAGE_EXTENSIONS.contains(&ext.as_str()) => {
            (clean(base), sniffed.to_string())
        }
        _ => (clean(stem), ext),
    };
    let stem = if stem.is_empty() {
        "texture".to_string()
    } else {
        stem
    };
    (stem, ext)
}

/// `s` with every character but ASCII letters, digits, `-`, `_` and `.` made `_`,
/// and no leading or trailing `.`.
fn clean(s: &str) -> String {
    let s: String = s
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '_',
        })
        .collect();
    s.trim_matches('.').to_string()
}

/// The extension `bytes`' magic number says they are, for the formats a name
/// alone may not tell: PNG and JPEG.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else {
        None
    }
}

/// `<stem>.<ext>`, or `<stem>.1.<ext>`, … when a name already `taken` matches it
/// ignoring case (a macOS disk would make the two one file).
fn unique(stem: &str, ext: &str, taken: &[String]) -> String {
    let free = |name: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(name));
    let base = format!("{stem}.{ext}");
    if free(&base) {
        return base;
    }
    (1..)
        .map(|n| format!("{stem}.{n}.{ext}"))
        .find(|name| free(name))
        .unwrap_or(base)
}

/// `target` as a `/`-separated, percent-encoded URI from directory `from`, both
/// resolved first.
pub fn relative_uri(from: &Path, target: &Path) -> Option<String> {
    let from = paths::canonicalize(from).ok()?;
    let target = paths::canonicalize(target).ok()?;
    let from: Vec<Component> = from.components().collect();
    let target: Vec<Component> = target.components().collect();
    let shared = from.iter().zip(&target).take_while(|(a, b)| a == b).count();
    let up = std::iter::repeat_n("..".to_string(), from.len() - shared);
    let down = target[shared..]
        .iter()
        .map(|c| c.as_os_str().to_string_lossy().into_owned());
    Some(uri::encode_path(
        &up.chain(down).collect::<Vec<_>>().join("/"),
    ))
}

#[cfg(test)]
mod tests;
