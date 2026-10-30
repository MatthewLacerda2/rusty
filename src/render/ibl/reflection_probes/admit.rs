//! Which cubes the probe array admits (#1245): a power of two up to [`MAX_FACE`],
//! checked before a cube is uploaded, and again on the frame's way in.

use std::rc::Rc;

use super::{ProbeArray, MAX_FACE};
use crate::render::ibl::cubemap::CubemapTexture;
use crate::scene::ReflectionProbe;

impl ProbeArray {
    /// Whether a `size`-texel cube at `path` fits the array: a power of two up to
    /// [`MAX_FACE`]; any other cube is warned about. The cubemap cache asks once per
    /// load of a path, before uploading it (#1245), so a cube that doesn't fit warns
    /// once, and again after a re-bake re-reads it.
    pub(in crate::render::ibl) fn admits(path: &str, size: u32) -> bool {
        if fits(MAX_FACE, size) {
            return true;
        }
        log::warn!(
            "[Reflection] cubemap {path} is {size} texels, not a power of two up to \
             {MAX_FACE}; its objects reflect the sky"
        );
        false
    }
}

/// Whether a `size`-texel cube fits an array of `face` texels: `face` halved a whole
/// number of times.
fn fits(face: u32, size: u32) -> bool {
    size > 0 && face.is_multiple_of(size) && (face / size).is_power_of_two()
}

/// The cubes of `wanted` that fit the array. Each should, being admitted before it
/// was uploaded: a debug build asserts it, a release build leaves out one that doesn't.
pub(super) fn admitted<'a>(
    wanted: &'a [(&'a ReflectionProbe, Rc<CubemapTexture>)],
) -> Vec<(&'a ReflectionProbe, &'a CubemapTexture)> {
    let face = MAX_FACE;
    debug_assert!(
        wanted.iter().all(|(_, cube)| fits(face, cube.size)),
        "every wanted cube was admitted"
    );
    (wanted.iter())
        .filter(|(_, cube)| fits(face, cube.size))
        .map(|(p, c)| (*p, &**c))
        .collect()
}
