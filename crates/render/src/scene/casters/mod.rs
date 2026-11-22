mod bvh;

use std::collections::BTreeSet;

pub(crate) use bvh::CasterIndex;
use glam::Mat4;

use super::{RenderObject, RenderScene};
use crate::passes::shadows::{casting_key, StaticsKey};
use crate::Frustum;
use rusty_scene::Scene;

/// Strays past which, plus an eighth of the entries, the BVH is rebuilt.
const STRAYS: usize = 64;

#[derive(Default)]
pub(super) struct Gathering {
    index: CasterIndex,
    key: Option<StaticsKey>,
    dynamic: BTreeSet<u32>,
    unresident: BTreeSet<u32>,
}

impl Gathering {
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

    pub(super) fn touch(&mut self, id: u32) {
        self.index.touch(id);
    }

    pub(super) fn stale(&mut self) {
        self.key = None;
    }
}

impl RenderScene {
    pub(super) fn refold_casting(&mut self) {
        if self.casting.is_none() {
            let statics = self.objects().filter(|(_, o)| o.is_static);
            let statics = statics.map(|(id, o)| (id, &o.casting));
            self.casting = Some(casting_key(statics, &self.unbaked));
        }
    }

    pub(crate) fn statics_key(&self, scene: &Scene) -> StaticsKey {
        let casting = self.casting.expect("an extract folds the digest");
        (scene.id(), scene.world.static_revision(), casting)
    }

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

    pub(crate) fn caster_index(&self) -> &CasterIndex {
        &self.casters.index
    }

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

    pub(crate) fn walls(&self) -> bool {
        let mut unresident = self.casters.unresident.iter();
        unresident.all(|&id| !self.occlusion.occludes(id))
    }
}

#[cfg(test)]
mod gather_gpu_tests;
