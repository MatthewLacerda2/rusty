use crate::lod::LodSelection;
use crate::{Frustum, Renderer, SmallObjectCull};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct CameraCulls {
    pub tested: u32,
    pub culled: u32,
    pub size_culled: u32,
    pub lod_hidden: u32,
    pub occluded: u32,
}

pub(crate) struct ViewCull<'a> {
    pub mask: u32,
    pub lod: &'a LodSelection,
    pub frustum: &'a Frustum,
    pub small: SmallObjectCull,
    pub occlusion: Option<glam::Vec3>,
}

impl Renderer {
    pub(crate) fn small_object_cull(&self, cam: &rusty_scene::Camera) -> SmallObjectCull {
        match self.capture.static_only {
            true => SmallObjectCull::off(),
            false => SmallObjectCull::from_camera(cam),
        }
    }

    pub(crate) fn occlusion_eye(&self, cam: &rusty_scene::Camera) -> Option<glam::Vec3> {
        let culls = rusty_scene::lighting::occlusion::culls_from(cam.projection, cam.near);
        (cam.use_occlusion_culling && culls && !self.capture.static_only).then_some(cam.position)
    }
}
