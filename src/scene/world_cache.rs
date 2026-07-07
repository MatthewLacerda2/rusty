//! src/scene/world_cache.rs — world-transform resolution and its per-frame cache (#331).
//!
//! The render frame used to resolve world matrices O(consumers × N × depth): the forward
//! pass, the static shadow collect, and the dynamic shadow collect each walked every
//! entity's parent chain independently, and a parent shared by K children had its whole
//! ancestor chain recomputed K times by *each* consumer. This module gives the scene ONE
//! authoritative store, filled O(N) once per frame in hierarchy order (parents before
//! children), that every render-frame consumer reads instead — the forward pass, both
//! shadow collects, and #330's frustum culling all share the single fill.
//!
//! **Freshness contract.** [`Scene::world_matrix`] returns an entity's world transform *as
//! of the most recent [`Scene::refresh_world_matrices`]*. The render path refreshes once,
//! before the camera stack, after the sim tick has settled every transform — so a transform
//! a script wrote earlier in the tick is observed by the frame's rendering. A read whose id
//! is missing from the store (an entity spawned since the last refresh, or an edit-mode
//! caller that never refreshes) transparently falls back to the live recursive walk, so a
//! cache miss is never stale-wrong — only uncached.
//!
//! **Why the collider refresh stays live.** [`Scene::update_entity_collider`] deliberately
//! keeps the recursive walk rather than reading this cache: it is called mid-tick by
//! scattered writers (script `Transform.Set*`, the editor inspector/gizmo, the physics
//! write-back), each needing the parent's transform *as just written this tick*, which a
//! once-per-frame cache refreshed at render time cannot provide without per-write
//! invalidation (the riskier dirty-flag shape the issue flags). Those callers walk only a
//! parent chain per moved entity, not all of N — the O(consumers × N × depth) blow-up the
//! cache removes was the render/shadow path, and that is where the cache is spent.
//!
//! **Determinism.** The cached values are a pure function of the same transforms, filled in
//! a deterministic id order, so replay stays byte-identical and the determinism guard is
//! unaffected. `scene` is single-threaded (it lives behind `Rc<RefCell>`), so the inner
//! `RefCell` never contends.

use std::cell::RefCell;
use std::collections::HashMap;

use glam::Mat4;

use crate::scene::Scene;

/// The scene's authoritative per-frame world-matrix store (#331). Interior-mutable so a
/// `&Scene` render path can refill it at a defined frame point without threading `&mut`
/// through every consumer.
#[derive(Default)]
pub struct WorldMatrixCache {
    matrices: RefCell<HashMap<u32, Mat4>>,
}

impl WorldMatrixCache {
    /// This entity's cached world matrix, or `None` if it was not in the last fill.
    fn get(&self, id: u32) -> Option<Mat4> {
        self.matrices.borrow().get(&id).copied()
    }

    /// Replace the whole store with a freshly computed `id -> world matrix` map.
    fn replace(&self, map: HashMap<u32, Mat4>) {
        *self.matrices.borrow_mut() = map;
    }
}

impl Scene {
    /// Resolve an entity's world transform by walking its parent chain live. This is the
    /// O(depth) recursive computation that [`Scene::refresh_world_matrices`] wraps; it
    /// survives as the cache-fill primitive and as the accessor for one-off edit-mode
    /// callers (picking, inspector, the snapshot API) that read outside a refreshed frame
    /// and must see the current transforms, not a frame-old cache.
    pub fn compute_world_matrix(&self, entity_id: u32) -> Mat4 {
        if let Some(transform) = self.world.transform(entity_id) {
            let local_mat = transform.to_matrix();
            drop(transform);
            if let Some(parent_id) = self.world.parent_id(entity_id) {
                self.compute_world_matrix(parent_id) * local_mat
            } else {
                local_mat
            }
        } else {
            Mat4::IDENTITY
        }
    }

    /// Fill the per-frame world-matrix store O(N) in hierarchy order (#331). Memoizes each
    /// entity's world matrix into `map` so a parent shared by many children — and every
    /// ancestor above it — is resolved exactly once, not once per child per consumer.
    /// Call this once at each frame point whose consumers read [`Scene::world_matrix`].
    pub fn refresh_world_matrices(&self) {
        let ids = self.world.ids().to_vec();
        let mut map: HashMap<u32, Mat4> = HashMap::with_capacity(ids.len());
        for id in ids {
            self.fill_world_matrix(id, &mut map);
        }
        self.world_cache.replace(map);
    }

    /// Memoized world-matrix fill for one entity: returns a cached value if present, else
    /// resolves the parent (recursively, sharing the same `map`) and stores the product.
    fn fill_world_matrix(&self, id: u32, map: &mut HashMap<u32, Mat4>) -> Mat4 {
        if let Some(cached) = map.get(&id) {
            return *cached;
        }
        let Some(local) = self.world.transform(id).map(|t| t.to_matrix()) else {
            return Mat4::IDENTITY;
        };
        let parent = self.world.parent_id(id);
        let world = match parent {
            Some(parent_id) => self.fill_world_matrix(parent_id, map) * local,
            None => local,
        };
        map.insert(id, world);
        world
    }

    /// An entity's world matrix as of the last [`Scene::refresh_world_matrices`] (#331).
    /// Falls back to the live recursive walk on a cache miss, so the value is always
    /// correct — see the module's freshness contract.
    pub fn world_matrix(&self, entity_id: u32) -> Mat4 {
        self.world_cache
            .get(entity_id)
            .unwrap_or_else(|| self.compute_world_matrix(entity_id))
    }

    /// Refresh one entity's collider world-AABB from its parent's *live* world matrix and
    /// its own local transform. Uses the recursive walk, not the per-frame cache, so a
    /// mid-tick writer (a script's `Transform.Set*`, the editor, the physics write-back)
    /// sees the parent transform as just written this tick — see the module note on why the
    /// collider path stays live.
    pub fn update_entity_collider(&mut self, id: u32) {
        let parent_mat = self
            .world
            .parent_id(id)
            .map(|p| self.compute_world_matrix(p));
        let Some(local) = self.world.transform(id).map(|t| t.to_matrix()) else {
            return;
        };
        let world_matrix = match parent_mat {
            Some(parent) => parent * local,
            None => local,
        };
        if let Some(mut col) = self.world.collider_mut(id) {
            let (min, max) = col.calculate_world_aabb(world_matrix);
            col.aabb_min = min;
            col.aabb_max = max;
        }
    }

    /// Refresh every entity's collider world-AABB (editor/load batch operation).
    pub fn update_all_colliders(&mut self) {
        let ids = self.world.ids().to_vec();
        for id in ids {
            self.update_entity_collider(id);
        }
    }
}

#[cfg(test)]
#[path = "world_cache_tests.rs"]
mod world_cache_tests;
