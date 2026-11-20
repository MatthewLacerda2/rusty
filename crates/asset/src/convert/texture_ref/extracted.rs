//! A texture a conversion extracted, as an image of its own (#2365): an FBX's
//! embedded texture, a `.blend`'s packed image, written into the textures folder
//! beside the cached `.glb` (`stage`). It is compressed like any texture
//! (`texture_import`), but it lives in the import cache, so it has no sidecar to
//! hold its import settings: they are the defaults, typed by the slots its model's
//! glTF samples it in ([`settings`]).
//!
//! | slots | Texture Type | format |
//! |---|---|---|
//! | normal only | `NormalMap` | BC5 |
//! | occlusion only (red) | `SingleChannel` | BC4 |
//! | packed data only (metallic-roughness, specular) | `Default`, *sRGB* off | BC7 |
//! | any colour (albedo, emissive), or a mix | `Default` | BC7 |
//!
//! An image two kinds of slot share takes `Default`, which every slot reads
//! whole (`texture_import::fits`), *sRGB* on when one of them is colour. A
//! sidecar someone wrote beside it wins, as for any image, until the next
//! conversion replaces its folder.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusty_core::fs::written::{written_since, Stale};

use crate::convert::{glb, is_import_cache, textures_owner};
use crate::gltf::uri;
use crate::sidecar::{TextureSettings, TextureType};

/// The cached glTF whose textures folder holds the image at `image`, made or
/// not: `None` for any image that is not one a conversion extracted (in a
/// project's import cache, or the temp folder's outside one).
pub fn glb_of(image: &Path) -> Option<PathBuf> {
    let folder = image.parent()?;
    let owner = textures_owner(folder.file_name()?.to_str()?)?;
    let in_cache = is_import_cache(image) || image.starts_with(temp_cache());
    in_cache.then(|| folder.with_file_name(owner))
}

/// Where a conversion outside a project writes (`convert::temp_path`).
fn temp_cache() -> PathBuf {
    std::env::temp_dir().join("rusty-import")
}

/// The images extracted beside the cached glTF at `glb` that it names, sorted:
/// what an import compresses for the model. Empty when it has none or is absent.
pub fn images(glb: &Path) -> Vec<PathBuf> {
    let base = glb.parent().unwrap_or(Path::new(""));
    let mut found: Vec<PathBuf> = glb::image_files(glb)
        .unwrap_or_default()
        .iter()
        .filter(|u| !u.starts_with("data:"))
        .filter_map(|u| uri::file(base, u))
        .filter(|f| crate::is_image_path(f) && f.is_file())
        .filter(|f| glb_of(f).is_some_and(|g| same(&g, glb)))
        .collect();
    found.sort();
    found.dedup();
    found
}

/// Whether two spellings name one file: equal once made absolute and folded.
fn same(a: &Path, b: &Path) -> bool {
    absolute(a) == absolute(b)
}

fn absolute(path: &Path) -> PathBuf {
    let made = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    rusty_core::paths::fold(&made)
}

/// The source the extracted image at `image` was converted from, when its cache
/// is a project's (`convert::cache_path`): `None` outside a project, whose temp
/// cache does not name it.
pub fn model_source(image: &Path) -> Option<PathBuf> {
    let glb = rusty_core::paths::to_slash(&glb_of(image)?);
    let marker = format!("{}/", crate::convert::IMPORT_CACHE_DIR);
    let at = glb.find(&marker)?;
    let rel = glb[at + marker.len()..].strip_suffix(".glb")?;
    Some(PathBuf::from(format!("{}{rel}", &glb[..at])))
}

/// The import settings of the extracted image at `image`, by the slots its
/// model's glTF samples it in (the module doc): `None` when it is not one. Each
/// glTF's table is read once and kept until the write log names the `.glb` (a
/// conversion notes it, `stage::note`), as `resolved` keeps its tables. An image
/// the table does not hold re-reads it once (another process may have written
/// the `.glb`); still absent, it takes the defaults, warned of once.
pub fn settings(image: &Path) -> Option<TextureSettings> {
    let glb = absolute(&glb_of(image)?);
    let image = absolute(image);
    let mut tables = TABLES.lock().unwrap_or_else(|e| e.into_inner());
    tables.forget_written();
    let held = |t: &Tables| t.by_glb.get(&glb).and_then(|f| f.get(&image)).copied();
    if let Some(found) = held(&tables) {
        return Some(found);
    }
    tables.by_glb.insert(glb.clone(), read(&glb));
    if let Some(found) = held(&tables) {
        return Some(found);
    }
    if tables.warned.insert(image.clone()) {
        log::warn!(
            "[Import] {}: {} samples it in no material slot; it imports with the default settings",
            image.display(),
            glb.display()
        );
    }
    Some(TextureSettings::default())
}

/// Each glTF's settings by extracted image (both absolute), and the images
/// warned of as in none of its slots.
struct Tables {
    /// The write generation already seen (`core::fs::written`).
    seen: u64,
    by_glb: BTreeMap<PathBuf, BTreeMap<PathBuf, TextureSettings>>,
    warned: BTreeSet<PathBuf>,
}

impl Tables {
    /// Drop the tables whose `.glb` was written. Allocates nothing when nothing was.
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
    warned: BTreeSet::new(),
});

fn read(glb: &Path) -> BTreeMap<PathBuf, TextureSettings> {
    let base = glb.parent().unwrap_or(Path::new(""));
    glb::image_slots(glb)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(u, slots)| Some((absolute(&uri::file(base, &u)?), of_slots(&slots))))
        .collect()
}

/// The settings an image sampled in `slots` imports with (the module doc's table).
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
