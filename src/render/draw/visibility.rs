//! One camera's culling inputs and counters (#330, #433, #1714): what the retained
//! scene's per-camera cull (`render::scene::draw`) reads and counts, and where the
//! small-object cull measures from.

use crate::render::lod::LodSelection;
use crate::render::{Frustum, Renderer, SmallObjectCull};

/// What one camera's culling skipped, by reason (#433's counters).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CameraCulls {
    /// Mesh entities tested against the frustum (#1714): every one the camera draws
    /// at all whose LOD level it shows.
    pub tested: u32,
    /// Outside the frustum (#330).
    pub culled: u32,
    /// Inside it, but too far or too small (#1714).
    pub size_culled: u32,
    /// Hidden because their LOD level is not the one shown (#472).
    pub lod_hidden: u32,
}

/// One camera's culling inputs: its culling mask (#92), the LOD levels it shows, its
/// frustum and its small-object cull.
pub(crate) struct ViewCull<'a> {
    pub mask: u32,
    pub lod: &'a LodSelection,
    pub frustum: &'a Frustum,
    pub small: SmallObjectCull,
}

impl Renderer {
    /// Where `cam`'s small-object cull measures from (#1714): `cam` itself, or
    /// nowhere during a capture, whose bake sees every prop whatever its size.
    pub(crate) fn small_object_cull(&self, cam: &crate::scene::Camera) -> SmallObjectCull {
        match self.capture.static_only {
            true => SmallObjectCull::off(),
            false => SmallObjectCull::from_camera(cam),
        }
    }
}
