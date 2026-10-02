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

use glam::Mat4;

use crate::ecs::Epoch;
use crate::scene::Scene;

/// The scene's authoritative per-frame world-matrix store (#331). Interior-mutable so a
/// `&Scene` render path can refill it at a defined frame point without threading `&mut`
/// through every consumer.
///
/// **Dense by id, kept across frames (#727).** Like `ecs::handles` (#696), `slots` is a
/// `Vec` indexed by stable id, so a read or a memo probe is a bounds check and a load
/// rather than a SipHash. A slot points into `matrices`, which holds one matrix per entity
/// filled this pass; both keep their allocation from frame to frame. Nothing is cleared
/// between fills: each refresh bumps `fill`, and a slot stamped with an older fill counts
/// as empty. Slots cost 8 bytes per id up to the highest id seen (ids are compact, see
/// `ecs::handles`), the matrices 64 bytes per live entity.
///
/// **Never stale-wrong across despawns or id reuse.** The fill also records the world's
/// [`Epoch`], which changes on every despawn and `clear` (after which ids restart at 1).
/// Until the next refresh, a read under a different epoch misses and the caller walks the
/// live hierarchy instead, so a gone or reused id is never answered from the old fill.
#[derive(Default)]
pub struct WorldMatrixCache {
    inner: RefCell<Store>,
}

#[derive(Default)]
struct Store {
    /// Stable id → `(fill, index into matrices)`; valid only when `fill` is current.
    slots: Vec<(u32, u32)>,
    matrices: Vec<Mat4>,
    /// The current fill's stamp; `0` means "never filled", so a fresh slot is empty.
    fill: u32,
    /// The world epoch the current fill was computed under.
    epoch: Option<Epoch>,
}

impl Store {
    /// The matrix filled for `id` this pass, if any.
    #[inline]
    fn get(&self, id: u32) -> Option<Mat4> {
        match self.slots.get(id as usize) {
            Some(&(fill, at)) if fill == self.fill => Some(self.matrices[at as usize]),
            _ => None,
        }
    }

    fn put(&mut self, id: u32, world: Mat4) {
        let slot = id as usize;
        if slot >= self.slots.len() {
            self.slots.resize(slot + 1, (0, 0));
        }
        self.slots[slot] = (self.fill, self.matrices.len() as u32);
        self.matrices.push(world);
    }

    /// Start a new fill under `epoch`: every slot of the previous one goes stale at once.
    fn begin(&mut self, epoch: Epoch) {
        self.fill = self.fill.wrapping_add(1);
        if self.fill == 0 {
            // Wrapped (after ~4 billion refreshes): old stamps could alias, so forget them.
            self.slots.clear();
            self.fill = 1;
        }
        self.matrices.clear();
        self.epoch = Some(epoch);
    }
}

impl WorldMatrixCache {
    /// This entity's cached world matrix, or `None` if it was not in the last fill or the
    /// world's ids have changed meaning since (a despawn or a `clear`).
    fn get(&self, id: u32, epoch: Epoch) -> Option<Mat4> {
        let store = self.inner.borrow();
        if store.epoch != Some(epoch) {
            return None;
        }
        store.get(id)
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
    /// entity's world matrix into the store so a parent shared by many children — and every
    /// ancestor above it — is resolved exactly once, not once per child per consumer.
    /// Call this once at each frame point whose consumers read [`Scene::world_matrix`].
    pub fn refresh_world_matrices(&self) {
        let mut store = self.world_cache.inner.borrow_mut();
        store.begin(self.world.epoch());
        for &id in self.world.ids() {
            self.fill_world_matrix(id, &mut store);
        }
    }

    /// Memoized world-matrix fill for one entity: returns the value already filled this
    /// pass if present, else resolves the parent (recursively, sharing the same store) and
    /// stores the product.
    fn fill_world_matrix(&self, id: u32, store: &mut Store) -> Mat4 {
        if let Some(cached) = store.get(id) {
            return cached;
        }
        let Some((local, parent)) = self.world.local_and_parent(id) else {
            return Mat4::IDENTITY;
        };
        let world = match parent {
            Some(parent_id) => self.fill_world_matrix(parent_id, store) * local,
            None => local,
        };
        store.put(id, world);
        world
    }

    /// An entity's world matrix as of the last [`Scene::refresh_world_matrices`] (#331).
    /// Falls back to the live recursive walk on a cache miss, so the value is always
    /// correct — see the module's freshness contract.
    pub fn world_matrix(&self, entity_id: u32) -> Mat4 {
        self.world_cache
            .get(entity_id, self.world.epoch())
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
