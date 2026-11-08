//! A scene's static shadow casters: their digest and the static bakes' key
//! (`RenderScene::statics_key`, part 2 of #1706), and the BVH their sweeps find
//! their candidates through (#1874, `bvh.rs`).
//!
//! Unity culls each shadow view over the scene's spatial structure, Unreal through
//! `FScene`'s primitive octree. Here a sweep gathers only the objects its light
//! volumes could reach ([`RenderScene::casters_near`]): the static ones the BVH
//! reaches, the static ones it cannot answer for, and every dynamic one, in the
//! order a walk of every object gives. The BVH is rebuilt when the scene's
//! [`StaticsKey`] moves or too many static objects strayed from it.

mod bvh;

pub(crate) use bvh::CasterIndex;
use glam::Mat4;

use super::{RenderObject, RenderScene};
use crate::render::passes::shadows::{casting_key, StaticsKey};
use crate::render::Frustum;
use crate::scene::Scene;

/// Strays (beyond an eighth of the entries) past which the BVH is rebuilt.
const STRAYS: usize = 64;

impl RenderScene {
    /// The digest, refolded if forgotten.
    pub(super) fn refold_casting(&mut self) {
        if self.casting.is_none() {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let statics = statics.map(|(id, o)| (id, &o.casting));
            self.casting = Some(casting_key(statics, &self.unbaked));
        }
    }

    /// The static bakes' key for `scene` (whose objects these are) now: what a
    /// cascade layer or an atlas tile keeps beside what it baked.
    pub(crate) fn statics_key(&self, scene: &Scene) -> StaticsKey {
        let casting = self.casting.expect("an extract folds the digest");
        (scene.id(), scene.world.static_revision(), casting)
    }

    /// The BVH rebuilt over the static objects' bounds if `scene`'s statics key
    /// moved since it was built, or too many static objects strayed (#1874).
    pub(super) fn refresh_casters(&mut self, scene: &Scene) {
        let key = self.statics_key(scene);
        let strayed = self.casters.strays() > STRAYS + self.casters.len() / 8;
        if self.casters_key != Some(key) || strayed {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            self.casters = CasterIndex::build(statics.map(|(id, o)| (id, o.bounds)));
            self.casters_key = Some(key);
        }
    }

    /// The static objects' BVH.
    pub(crate) fn caster_index(&self) -> &CasterIndex {
        &self.casters
    }

    /// Every object a shadow volume of `view_projs` could reach, with its id, in
    /// [`Self::objects`]' order: a superset of what each volume's frustum keeps.
    pub(crate) fn casters_near(&self, view_projs: &[Mat4]) -> Vec<(u32, &RenderObject)> {
        let frustums: Vec<Frustum> = view_projs
            .iter()
            .map(|&v| Frustum::from_view_proj(v))
            .collect();
        let mut ids = self.casters.near(&frustums);
        ids.extend(self.dynamic.iter().copied());
        let mut near: Vec<(u32, &RenderObject)> = ids
            .into_iter()
            .filter_map(|id| self.objects.get(&id).map(|o| (id, o)))
            .collect();
        near.sort_unstable_by_key(|(_, o)| o.seq);
        near.dedup_by_key(|(id, _)| *id);
        near
    }

    /// Whether every baked occluder unchanged since the bake has its buffers: one
    /// that has none draws into no map, so no light may cull by the PVS (#1714).
    pub(crate) fn walls(&self) -> bool {
        let occlusion = &self.occlusion;
        let mut objects = self.objects.iter();
        objects.all(|(&id, o)| o.resident || !occlusion.occludes(id))
    }
}

#[cfg(test)]
mod gather_gpu_tests;
