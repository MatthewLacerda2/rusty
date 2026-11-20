//! Where an image's compressed copy lives (#1574): in the import cache, beside
//! converted models, named as its source plus [`CACHE_SUFFIX`]; for a texture a
//! conversion extracted, beside its model's cache (#2365, [`extracted_cache`]).

use std::path::{Path, PathBuf};

use crate::convert::texture_ref::extracted;

/// Where the import cache keeps the image at `source`'s compressed copy: beside
/// its model's cache for a texture a conversion extracted ([`extracted_cache`]).
pub fn cache_file(source: &Path) -> Result<PathBuf, String> {
    if let Some(glb) = extracted::glb_of(source) {
        return Ok(extracted_cache(&glb, source));
    }
    crate::convert::cache_location_as(source, CACHE_SUFFIX).map_err(|e| e.to_string())
}

/// [`cache_file`] of the image `image` (relative to `root`) of the project at
/// `root`, wherever the working directory is: what an export copies.
pub fn cache_path_in(root: &Path, image: &Path) -> PathBuf {
    match extracted::glb_of(&root.join(image)) {
        Some(glb) => extracted_cache(&glb, image),
        None => crate::convert::cache_path_as(root, image, CACHE_SUFFIX),
    }
}

/// Where the compressed copy of `image`, extracted beside the cached glTF at
/// `glb`, lives: `<glb>.bctex/<image's name>.bctex`. Not in the textures folder,
/// whose name each conversion changes (`convert::stage`), so a re-conversion that
/// leaves an image's bytes as they were finds its copy current and re-encodes
/// nothing.
pub fn extracted_cache(glb: &Path, image: &Path) -> PathBuf {
    let mut folder = glb.as_os_str().to_owned();
    folder.push(CACHE_SUFFIX);
    let mut name = image.file_name().unwrap_or_default().to_owned();
    name.push(CACHE_SUFFIX);
    PathBuf::from(folder).join(name)
}

/// What a compressed copy's name adds to its source's.
pub const CACHE_SUFFIX: &str = ".bctex";
