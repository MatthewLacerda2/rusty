//! Compressed copies live in the import cache, named source plus [`CACHE_SUFFIX`]; an extracted
//! texture's beside its model's cache.

use std::path::{Path, PathBuf};

use crate::convert::texture_ref::extracted;

pub fn cache_file(source: &Path) -> Result<PathBuf, String> {
    if let Some(glb) = extracted::glb_of(source) {
        return Ok(extracted_cache(&glb, source));
    }
    crate::convert::cache_location_as(source, CACHE_SUFFIX).map_err(|e| e.to_string())
}

pub fn cache_path_in(root: &Path, image: &Path) -> PathBuf {
    match extracted::glb_of(&root.join(image)) {
        Some(glb) => extracted_cache(&glb, image),
        None => crate::convert::cache_path_as(root, image, CACHE_SUFFIX),
    }
}

/// `<glb>.bctex/<image's name>.bctex`, not in the textures folder, whose name each conversion
/// changes (`convert::stage`), so an unchanged image finds its copy current.
pub fn extracted_cache(glb: &Path, image: &Path) -> PathBuf {
    let mut folder = glb.as_os_str().to_owned();
    folder.push(CACHE_SUFFIX);
    let mut name = image.file_name().unwrap_or_default().to_owned();
    name.push(CACHE_SUFFIX);
    PathBuf::from(folder).join(name)
}

pub const CACHE_SUFFIX: &str = ".bctex";
