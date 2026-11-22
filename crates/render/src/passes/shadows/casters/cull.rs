use glam::Mat4;

use super::buffer::CasterData;
use super::{CasterFrame, CasterKey};
use crate::scene::{OcclusionBinding, RenderObject};
use crate::Frustum;

pub(super) struct Candidate {
    pub id: u32,
    pub key: CasterKey,
    pub caster: CasterData,
    pub bounds: Option<(glam::Vec3, glam::Vec3)>,
    pub small: bool,
    /// Its map takes the faces turned from the light, which the PVS bake never aims at.
    pub two_sided: bool,
}

impl CasterFrame<'_> {
    pub(super) fn culls_small(
        &self,
        object: &RenderObject,
        bounds: Option<(glam::Vec3, glam::Vec3)>,
    ) -> bool {
        bounds.is_some_and(|(lo, hi)| {
            let centre = (lo + hi) * 0.5;
            let (limits, local) = (object.small, object.local_aabb);
            self.small.culls_by(limits, centre, local, object.world)
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShadowCulls {
    pub tested: u32,
    pub frustum: u32,
    pub size: u32,
    pub occlusion: u32,
    pub gather_us: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::passes::shadows) struct LightVolume {
    pub view_proj: Mat4,
    pub row: Option<u32>,
}

impl LightVolume {
    pub(in crate::passes::shadows) fn open(view_proj: Mat4) -> Self {
        Self {
            view_proj,
            row: None,
        }
    }

    pub(in crate::passes::shadows) fn resident(self, walls: bool) -> Self {
        match walls {
            true => self,
            false => Self::open(self.view_proj),
        }
    }
}

/// Culled by the light's volume, not the camera's: an off-screen caster still shadows. Only the
/// size verdict is the camera's.
pub(super) fn cull<'a>(
    candidates: impl IntoIterator<Item = &'a Candidate>,
    volume: LightVolume,
    pvs: &OcclusionBinding,
    culls: &mut ShadowCulls,
) -> Vec<(CasterKey, CasterData)> {
    let frustum = Frustum::from_view_proj(volume.view_proj);
    let mut hidden = false;
    let mut kept = Vec::new();
    let mut last = None;
    for c in candidates {
        let first_run = last != Some(c.id);
        last = Some(c.id);
        let inside = c
            .bounds
            .is_none_or(|(lo, hi)| frustum.intersects_aabb(lo, hi));
        let drawn = inside && !c.small;
        if first_run {
            hidden = drawn && !c.two_sided && volume.row.is_some_and(|r| pvs.hides(r, c.id));
            culls.tested += 1;
            culls.frustum += u32::from(!inside);
            culls.size += u32::from(inside && c.small);
            culls.occlusion += u32::from(hidden);
        }
        if drawn && !hidden {
            kept.push((c.key.clone(), c.caster));
        }
    }
    kept
}
