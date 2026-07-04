//! src/scene/world_matrix.rs — the per-frame world-matrix cache and the collider-AABB
//! refresh built on it (#331).
//!
//! One authoritative world-transform store for the whole frame: [`Scene::rebuild_world_matrices`]
//! fills it O(N) in hierarchy order at fixed frame points (render start, physics write-back),
//! and every per-frame consumer — the solid pass, both shadow collects, light placement, the
//! physics collider refresh — reads it via [`Scene::world_matrix`] instead of walking the
//! parent chain itself. This replaces the old O(consumers × N × depth) pattern where each
//! consumer re-resolved every entity's ancestor chain independently. The recursive
//! [`Scene::compute_world_matrix`] survives as the fill's semantics and as the always-fresh
//! fallback for one-off edit-mode callers (picking, inspector, snapshot) outside a cache window.

use std::collections::HashMap;

use glam::Mat4;

use crate::scene::Scene;

impl Scene {
    /// Resolve an entity's world matrix by recursively walking the parent chain — the
    /// always-fresh, uncached computation. It is O(depth) per call and O(N × depth) if
    /// called for every entity, so per-frame consumers must NOT use it directly: they
    /// read [`Scene::world_matrix`], which serves the once-per-frame cache. This method
    /// survives as the cache-fill primitive's semantics and as the correct fallback for
    /// one-off edit-mode callers (picking, inspector, snapshot, per-frame camera pick)
    /// that run outside a cache-valid window and need the live transform right now.
    pub fn compute_world_matrix(&self, entity_id: u32) -> Mat4 {
        if let Some(entity) = self.get_entity(entity_id) {
            let local_mat = entity.transform.to_matrix();
            if let Some(parent_id) = entity.parent_id {
                self.compute_world_matrix(parent_id) * local_mat
            } else {
                local_mat
            }
        } else {
            Mat4::IDENTITY
        }
    }

    /// Fill the per-frame world-matrix cache O(N) in a single pass (#331). Every live
    /// entity's world matrix is stored; a child reads its parent's already-memoized
    /// value, so a parent shared by K children is resolved once, not K times. The
    /// caller invokes this at a fixed frame point AFTER the latest transform writes —
    /// the start of a render (covers the solid pass, both shadow collects, and light
    /// placement) and the physics write-back once all bodies are integrated. That is
    /// the freshness contract: [`Scene::world_matrix`] reflects the transforms as of
    /// the most recent rebuild. Deterministic — a pure function of the same transforms
    /// in a fixed id order — so replay stays byte-identical.
    pub fn rebuild_world_matrices(&self) {
        let mut cache = self.world_matrices.borrow_mut();
        cache.clear();
        for &id in self.world.ids() {
            self.fill_world_matrix(id, &mut cache);
        }
    }

    /// Memoized fill for one entity: return the cached matrix if already computed this
    /// pass, else resolve `parent · local` (filling the parent first) and store it.
    fn fill_world_matrix(&self, id: u32, cache: &mut HashMap<u32, Mat4>) -> Mat4 {
        if let Some(m) = cache.get(&id) {
            return *m;
        }
        let Some((local, parent_id)) = self
            .get_entity(id)
            .map(|e| (e.transform.to_matrix(), e.parent_id))
        else {
            return Mat4::IDENTITY;
        };
        let world = match parent_id {
            Some(p) => self.fill_world_matrix(p, cache) * local,
            None => local,
        };
        cache.insert(id, world);
        world
    }

    /// The per-frame consumers' accessor: the entity's world matrix from the cache last
    /// filled by [`Scene::rebuild_world_matrices`], falling back to the fresh recursive
    /// walk on a miss (e.g. an entity spawned after the rebuild). This is what render,
    /// the shadow collects, light placement, and the physics collider refresh call
    /// instead of walking the chain themselves.
    pub fn world_matrix(&self, entity_id: u32) -> Mat4 {
        self.world_matrices
            .borrow()
            .get(&entity_id)
            .copied()
            .unwrap_or_else(|| self.compute_world_matrix(entity_id))
    }

    /// Refresh entity `id`'s collider AABB from the parent matrix, resolved by
    /// `parent_mat`. Shared spine of the two collider-refresh entry points below.
    fn refresh_collider_with(&mut self, id: u32, parent_mat: Option<Mat4>) {
        if let Some(mut entity) = self.get_entity_mut(id) {
            entity.update_collider(parent_mat);
        }
    }

    /// Refresh one entity's collider AABB using a freshly walked parent chain. This is
    /// the safe default for scattered single writes (script `Transform.Set*`, editor
    /// gizmo) where no cache-valid window is guaranteed.
    pub fn update_entity_collider(&mut self, id: u32) {
        let parent_id = self.get_entity(id).and_then(|e| e.parent_id);
        let parent_mat = parent_id.map(|p| self.compute_world_matrix(p));
        self.refresh_collider_with(id, parent_mat);
    }

    /// Refresh one entity's collider AABB using the CACHED parent matrix (#331). Only
    /// valid when [`Scene::rebuild_world_matrices`] has run since the last transform
    /// write — the physics write-back guarantees this by rebuilding once after all
    /// bodies are integrated, then refreshing every body's collider through here.
    pub fn refresh_collider_cached(&mut self, id: u32) {
        let parent_id = self.get_entity(id).and_then(|e| e.parent_id);
        let parent_mat = parent_id.map(|p| self.world_matrix(p));
        self.refresh_collider_with(id, parent_mat);
    }

    pub fn update_all_colliders(&mut self) {
        let ids = self.world.ids().to_vec();
        for id in ids {
            self.update_entity_collider(id);
        }
    }
}

#[cfg(test)]
#[path = "world_matrix_tests.rs"]
mod world_matrix_tests;
