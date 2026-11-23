//! An extracted texture has no sidecar, so its import settings are typed by the slots
//! its model's glTF samples it in (`of_slots`); a mix takes `Default`, which every slot
//! reads whole (`texture_import::fits`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusty_core::fs::written::{written_since, Stale};

use crate::convert::{glb, is_import_cache, textures_owner};
use crate::gltf::uri;
use crate::sidecar::{TextureSettings, TextureType};

pub fn glb_of(image: &Path) -> Option<PathBuf> {
    let folder = image.parent()?;
    let owner = textures_owner(folder.file_name()?.to_str()?)?;
    let in_cache = is_import_cache(image) || image.starts_with(temp_cache());
    in_cache.then(|| folder.with_file_name(owner))
}

fn temp_cache() -> PathBuf {
    std::env::temp_dir().join("rusty-import")
}

/// Sorted. An image in no slot the engine reads is never uploaded, so never encoded.
pub fn images(glb: &Path) -> Vec<PathBuf> {
    let base = glb.parent().unwrap_or(Path::new(""));
    let mut found: Vec<PathBuf> = glb::image_slots(glb)
        .unwrap_or_default()
        .keys()
        .filter(|u| !u.starts_with("data:"))
        .filter_map(|u| uri::file(base, u))
        .filter(|f| crate::is_image_path(f) && f.is_file())
        .filter(|f| glb_of(f).is_some_and(|g| same(&g, glb)))
        .collect();
    found.sort();
    found.dedup();
    found
}

fn same(a: &Path, b: &Path) -> bool {
    absolute(a) == absolute(b)
}

fn absolute(path: &Path) -> PathBuf {
    let made = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    rusty_core::paths::fold(&made)
}

/// `None` outside a project: the temp cache keeps no source path.
pub fn model_source(image: &Path) -> Option<PathBuf> {
    let glb = rusty_core::paths::to_slash(&glb_of(image)?);
    let marker = format!("{}/", crate::convert::IMPORT_CACHE_DIR);
    let at = glb.find(&marker)?;
    let rel = glb[at + marker.len()..].strip_suffix(".glb")?;
    Some(PathBuf::from(format!("{}{rel}", &glb[..at])))
}

/// Tables are kept until the write log names the `.glb`. An image a held table lacks
/// re-reads once (another process may have written it); still absent, it takes the
/// defaults, warned of once.
pub fn settings(image: &Path) -> Option<TextureSettings> {
    let glb = absolute(&glb_of(image)?);
    let image = absolute(image);
    let mut tables = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    tables.forget_written();
    let held = |t: &Tables| t.by_glb.get(&glb).and_then(|f| f.get(&image)).copied();
    if let Some(found) = held(&tables) {
        return Some(found);
    }
    let table = tables.by_glb.entry(glb.clone()).or_default();
    *table = read(&glb);
    if let Some(found) = table.get(&image) {
        return Some(*found);
    }
    log::warn!(
        "[Import] {}: {} uses it only in a material slot the engine does not read, or in none; \
         it imports with the default settings",
        image.display(),
        glb.display()
    );
    table.insert(image, TextureSettings::default());
    Some(TextureSettings::default())
}

/// Both paths absolute.
struct Tables {
    seen: u64,
    by_glb: BTreeMap<PathBuf, BTreeMap<PathBuf, TextureSettings>>,
}

impl Tables {
    fn forget_written(&mut self) {
        let (seen, written) = written_since(self.seen);
        self.seen = seen;
        if written.is_empty() {
            return;
        }
        let stale = Stale::new(written);
        self.by_glb
            .retain(|glb, _| !stale.matches(&glb.to_string_lossy()));
    }
}

static TABLES: Mutex<Tables> = Mutex::new(Tables {
    seen: 0,
    by_glb: BTreeMap::new(),
});

fn read(glb: &Path) -> BTreeMap<PathBuf, TextureSettings> {
    let base = glb.parent().unwrap_or(Path::new(""));
    glb::image_slots(glb)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(u, slots)| Some((absolute(&uri::file(base, &u)?), of_slots(&slots))))
        .collect()
}

pub fn of_slots(slots: &[glb::Slot]) -> TextureSettings {
    use glb::Slot;
    let only = |slot: Slot| !slots.is_empty() && slots.iter().all(|s| *s == slot);
    let (texture_type, srgb) = if only(Slot::Normal) {
        (TextureType::NormalMap, true)
    } else if only(Slot::Red) {
        (TextureType::SingleChannel, true)
    } else {
        let data = !slots.is_empty() && !slots.contains(&Slot::Colour);
        (TextureType::Default, !data)
    };
    TextureSettings {
        texture_type,
        srgb,
        ..TextureSettings::default()
    }
}
