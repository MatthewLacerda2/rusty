//! The gathered casters culled per light volume: by the light's frustum (#330), then
//! by the small-object verdict the gather took from the base camera (#1714), each
//! counted once per entity per volume for `Debug.Stats` (`shadow_cull_tested`, …);
//! and the gather's two per-entity tests, a variant cut and the small verdict.

use glam::Mat4;

use super::buffer::CasterData;
use super::{CasterFrame, CasterKey};
use crate::components::MeshComponent;
use crate::render::{Frustum, GpuMesh};

/// One caster run before culling: its entity, its draw, its instance data, its world
/// bounds (`None` when they cannot be bounded: never culled) and whether the base
/// camera finds its entity too far or too small (#1714). An entity's runs are
/// neighbours, and share the last two.
pub(super) struct Candidate {
    pub id: u32,
    pub key: CasterKey,
    pub caster: CasterData,
    pub bounds: Option<(glam::Vec3, glam::Vec3)>,
    pub small: bool,
}

impl CasterFrame<'_> {
    /// Whether a surface variant cuts any of entity `id`'s `slots` material slots:
    /// such a caster counts as dynamic (see `clips`).
    pub(super) fn moving(&self, id: u32, slots: usize) -> bool {
        (0..slots).any(|slot| {
            self.clips
                .get(&(id, slot))
                .is_some_and(|c| c.clip.is_variant())
        })
    }

    /// Whether the base camera culls `mesh` (on `gpu_mesh`, at `world`, inside
    /// `bounds`) as too far or too small (#1714); never when it cannot be bounded.
    pub(super) fn culls_small(
        &self,
        mesh: &MeshComponent,
        gpu_mesh: &GpuMesh,
        bounds: Option<(glam::Vec3, glam::Vec3)>,
        world: Mat4,
    ) -> bool {
        bounds.is_some_and(|(lo, hi)| {
            let centre = (lo + hi) * 0.5;
            self.small.culls(mesh, centre, gpu_mesh.local_aabb, world)
        })
    }
}

/// What the shadow views' culling did this frame, summed over every volume each
/// sweep drew: entities tested, culled by the light's frustum, and kept by it but
/// culled as too far or too small from the base camera.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ShadowCulls {
    pub tested: u32,
    pub frustum: u32,
    pub size: u32,
}

/// The casters `volume` (a light's view-projection) draws of `candidates`, its
/// verdicts added to `culls`. Culled against the LIGHT's volume, not the camera's: an
/// off-screen caster still inside a cascade keeps its shadow — culling casters by
/// the player camera's frustum is the classic pop-a-shadow bug. Only the size
/// verdict is the camera's, so a shadow goes with its prop.
pub(super) fn cull(
    candidates: &[Candidate],
    volume: Mat4,
    culls: &mut ShadowCulls,
) -> Vec<(CasterKey, CasterData)> {
    let frustum = Frustum::from_view_proj(volume);
    let mut kept = Vec::with_capacity(candidates.len());
    let mut last = None;
    for c in candidates {
        let first_run = last != Some(c.id);
        last = Some(c.id);
        let inside = c
            .bounds
            .is_none_or(|(lo, hi)| frustum.intersects_aabb(lo, hi));
        if first_run {
            culls.tested += 1;
            culls.frustum += u32::from(!inside);
            culls.size += u32::from(inside && c.small);
        }
        if inside && !c.small {
            kept.push((c.key.clone(), c.caster));
        }
    }
    kept
}
