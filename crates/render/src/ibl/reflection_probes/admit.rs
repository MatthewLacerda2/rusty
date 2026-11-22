use super::{ProbeArray, MAX_FACE};
use crate::ibl::cubemap::CubeShape;
use rusty_scene::ReflectionProbe;

impl ProbeArray {
    pub(in crate::ibl) fn admits(path: &str, size: u32) -> bool {
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

fn fits(face: u32, size: u32) -> bool {
    size > 0 && face.is_multiple_of(size) && (face / size).is_power_of_two()
}

pub(super) fn admitted<'a>(
    wanted: &[(&'a ReflectionProbe, CubeShape)],
) -> Vec<(&'a ReflectionProbe, CubeShape)> {
    let face = MAX_FACE;
    debug_assert!(
        wanted.iter().all(|(_, cube)| fits(face, cube.size)),
        "every wanted cube was admitted"
    );
    (wanted.iter())
        .filter(|(_, cube)| fits(face, cube.size))
        .copied()
        .collect()
}
