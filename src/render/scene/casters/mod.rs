//! A scene's static shadow casters: their digest and the static bakes' key
//! (`RenderScene::statics_key`, part 2 of #1706), and the BVH their sweeps find
//! their candidates through (#1874, `bvh.rs`).
//!
//! Unity culls each shadow view over the scene's spatial structure, Unreal through
//! `FScene`'s primitive octree. Here a sweep gathers only the objects its light
//! volumes could reach ([`RenderScene::casters_near`]), in the order a walk of every
//! object gives: a static bake the static ones the BVH reaches, a per-frame sweep
//! every dynamic object and the static ones that cast per frame (a `LODGroup`'s or
//! a small-culled renderer, a moving cut: a short list kept with the BVH), and both
//! the static ones the BVH cannot answer for. The BVH is rebuilt when the scene's
//! [`StaticsKey`] moves (the digest folds which statics are baked) or too many
//! static objects strayed from it.

mod bvh;

use std::collections::BTreeSet;

pub(crate) use bvh::CasterIndex;
use glam::Mat4;

use super::{RenderObject, RenderScene};
use crate::render::passes::shadows::{casting_key, StaticsKey};
use crate::render::Frustum;
use crate::scene::Scene;

/// Strays (beyond an eighth of the entries) past which the BVH is rebuilt.
const STRAYS: usize = 64;

/// What a scene's shadow sweeps gather through: the static objects' BVH and the
/// key it was built under (`None`: rebuilt on the next extract), the objects that
/// are not static, and those with no GPU buffers.
#[derive(Default)]
pub(super) struct Gathering {
    index: CasterIndex,
    key: Option<StaticsKey>,
    dynamic: BTreeSet<u32>,
    unresident: BTreeSet<u32>,
}

impl Gathering {
    /// Entity `id`'s object is now `object` (`None`: it has none).
    pub(super) fn set(&mut self, id: u32, object: Option<&RenderObject>) {
        self.dynamic.remove(&id);
        self.unresident.remove(&id);
        if let Some(o) = object {
            if !o.is_static {
                self.dynamic.insert(id);
            }
            if !o.resident {
                self.unresident.insert(id);
            }
        }
    }

    /// Static object `id` was re-derived or moved: the BVH no longer answers for it.
    pub(super) fn touch(&mut self, id: u32) {
        self.index.touch(id);
    }

    /// Which statics are baked may have moved: rebuilt on the next extract.
    pub(super) fn stale(&mut self) {
        self.key = None;
    }
}

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
        let index = &self.casters.index;
        let strayed = index.strays() > STRAYS + index.len() / 8;
        if self.casters.key != Some(key) || strayed {
            let unbaked = &self.unbaked;
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let statics = statics.map(|(id, o)| {
                let per_frame = o.moving || unbaked.holds(id, o.casting.culls_small());
                (id, o.bounds, per_frame)
            });
            self.casters.index = CasterIndex::build(statics);
            self.casters.key = Some(key);
        }
    }

    /// The static objects' BVH.
    pub(crate) fn caster_index(&self) -> &CasterIndex {
        &self.casters.index
    }

    /// Every object a shadow volume of `view_projs` could reach, with its id, in
    /// [`Self::objects`]' order: for a static bake (`baked`), the static ones; for
    /// a per-frame sweep, the dynamic ones and the statics that cast per frame.
    /// A superset of what the sweep takes and each volume's frustum keeps.
    pub(crate) fn casters_near(
        &self,
        view_projs: &[Mat4],
        baked: bool,
    ) -> Vec<(u32, &RenderObject)> {
        let gathering = &self.casters;
        let mut ids = match baked {
            true => {
                let frustums = view_projs.iter().map(|&v| Frustum::from_view_proj(v));
                gathering.index.near(&frustums.collect::<Vec<_>>())
            }
            false => {
                let mut ids = gathering.index.per_frame();
                ids.extend(gathering.dynamic.iter().copied());
                ids
            }
        };
        ids.extend(gathering.index.unanswered());
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
        let mut unresident = self.casters.unresident.iter();
        unresident.all(|&id| !self.occlusion.occludes(id))
    }
}

#[cfg(test)]
mod gather_gpu_tests;
