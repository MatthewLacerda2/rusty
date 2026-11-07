//! The gathered casters culled per light volume: by the light's frustum (#330), then
//! by the small-object verdict the gather took from the base camera, then by the
//! light's own PVS row (#1714), each counted once per entity per volume for
//! `Debug.Stats` (`shadow_cull_tested`, …); and the gather's small verdict.

use glam::Mat4;

use super::buffer::CasterData;
use super::{CasterFrame, CasterKey};
use crate::render::scene::{OcclusionBinding, RenderObject};
use crate::render::Frustum;

/// One caster run before culling: its entity, its draw, its instance data, its world
/// bounds (`None` when they cannot be bounded: never culled), whether the base
/// camera finds its entity too far or too small (#1714), and whether it casts
/// two-sided. An entity's runs are neighbours, and share the last three.
pub(super) struct Candidate {
    pub id: u32,
    pub key: CasterKey,
    pub caster: CasterData,
    pub bounds: Option<(glam::Vec3, glam::Vec3)>,
    pub small: bool,
    /// Cast mode `TwoSided` (#1571): its map takes the faces turned from the light,
    /// which the PVS bake never aims at, so the PVS never culls it.
    pub two_sided: bool,
}

impl CasterFrame<'_> {
    /// Whether the base camera culls `object` (inside `bounds`, its posed ones for
    /// a skinned caster) as too far or too small (#1714); never when it cannot be
    /// bounded.
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

/// What the shadow views' culling did this frame, summed over every volume each
/// sweep drew: entities tested, culled by the light's frustum, kept by it but
/// culled as too far or too small from the base camera, and kept by both but
/// hidden from the light's PVS cell; and the CPU µs the sweeps took to gather,
/// cull and batch their casters (wall-clock).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShadowCulls {
    pub tested: u32,
    pub frustum: u32,
    pub size: u32,
    pub occlusion: u32,
    pub gather_us: u64,
}

/// One light volume a sweep draws: its view-projection, and the PVS row of the
/// cell its light stands in (`OcclusionBinding::shadow_row`), `None` for a volume
/// that culls nothing by occlusion (a cascade, a shadowmasked tile, no usable bake).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::render::passes::shadows) struct LightVolume {
    pub view_proj: Mat4,
    pub row: Option<u32>,
}

impl LightVolume {
    /// A volume that culls nothing by occlusion.
    pub(in crate::render::passes::shadows) fn open(view_proj: Mat4) -> Self {
        Self {
            view_proj,
            row: None,
        }
    }
}

/// The casters `volume` draws of `candidates`, its verdicts added to `culls`.
/// Culled against the LIGHT's volume, not the camera's: an off-screen caster still
/// inside a cascade keeps its shadow — culling casters by the player camera's
/// frustum is the classic pop-a-shadow bug. Only the size verdict is the camera's,
/// so a shadow goes with its prop. A volume with a PVS row drops the baked casters
/// `pvs` says its light's cell cannot see: the wall between is in the same map.
pub(super) fn cull(
    candidates: &[Candidate],
    volume: LightVolume,
    pvs: &OcclusionBinding,
    culls: &mut ShadowCulls,
) -> Vec<(CasterKey, CasterData)> {
    let frustum = Frustum::from_view_proj(volume.view_proj);
    let mut hidden = false;
    let mut kept = Vec::with_capacity(candidates.len());
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
