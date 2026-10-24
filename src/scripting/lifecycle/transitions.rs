//! src/scripting/lifecycle/transitions.rs — the enable/disable/destroy dispatch.
//!
//! The #323 transition callbacks that don't ride the plain active-gated init
//! path in `mod.rs`:
//!   - `OnDisable` on the falling edge — an entity that was active is now
//!     inactive. Detected by a diff against the previous tick, so it needs the
//!     *inactive* set (`disabled_keys`), the mirror of `mod.rs`'s `eligible_keys`.
//!   - `OnDestroy` (preceded by `OnDisable` for still-active instances) on the
//!     deferred-destroy drain: `Scene.DestroyEntity` queued ids this tick, and
//!     `apply_pending_destroys` fires teardown, then removes the entities with
//!     their subtrees.
//!   - The same teardown for a whole outgoing scene on `Scene.Load` (#432):
//!     `unload_entities` fires it for every non-surviving entity.
//!
//! Both tear down in one order, [`teardown_order`]: children before parents
//! (#1298). Unity documents no order; the one public test (Uninomicon, Unity
//! 2017.4) saw `OnDisable` children first but `OnDestroy` parents first. With no
//! current source to settle `OnDestroy`, both passes run children first.
//!
//! Both reuse the shared dispatch core (`dispatch_keys`, `call_hook`,
//! `entity_active`) so they stay loops, not copies.

use std::cmp::Reverse;
use std::collections::BTreeSet;

use super::super::callbacks::{ON_DESTROY, ON_DISABLE};
use super::super::manager::{ScriptInstance, ScriptManager};
use crate::scene::{Despawn, Scene};

/// The order a teardown visits `ids` (ascending): deepest first, then ascending
/// id, so every child comes before its parent. The one order a destroy and an
/// unload share (#1298).
fn teardown_order(scene: &Scene, ids: &[u32]) -> Vec<u32> {
    let mut order = ids.to_vec();
    // Stable, so equal depths keep `ids`' ascending order.
    order.sort_by_cached_key(|&id| Reverse(scene.world.ancestors(id).count()));
    order
}

impl ScriptManager {
    /// The disable mirror of `eligible_keys`: ascending keys of instances passing
    /// `pred` whose owning entity still exists but is now inactive — the
    /// falling-edge set for `OnDisable`.
    fn disabled_keys(&self, pred: impl Fn(&ScriptInstance) -> bool) -> Vec<(u32, usize)> {
        self.entity_scripts
            .iter()
            .filter(|(&(id, _), inst)| pred(inst) && self.entity_active(id) == Some(false))
            .map(|(&key, _)| key)
            .collect()
    }

    /// The falling-edge sub-phase: `OnDisable(id)` on every awoken, still-enabled
    /// instance whose entity is now inactive, then clear `enabled_last` so the
    /// edge fires exactly once (a re-enable re-arms it via `OnEnable`). Called from
    /// `init_scripts`, so `pub(super)` for the parent module.
    pub(super) fn dispatch_disabled(&mut self) {
        let keys = self.disabled_keys(|i| i.awoken && i.enabled_last);
        self.dispatch_keys(&keys, ON_DISABLE);
        for &(id, _) in &keys {
            // Unity: a deactivated object's coroutines stop (#444) — timers too.
            self.cells
                .timers
                .borrow_mut()
                .retain_owners(|owner| owner != id);
        }
        for key in keys {
            if let Some(inst) = self.entity_scripts.get_mut(&key) {
                inst.enabled_last = false;
            }
        }
    }

    /// Drain the scene's deferred-destroy queue (filled by play-mode
    /// `Scene.DestroyEntity`) and dispatch the teardown (#323) of everything those
    /// destroys remove, children included (`Scene::destroy_set`, #1283), then
    /// despawn exactly that set and forget its script instances. The entities stay
    /// live through dispatch (so an `OnDestroy` can read its own transform).
    /// Cascading destroys requested from within an `OnDestroy` fall to the next
    /// tick's drain, since the queue was taken up front. Returns what was removed,
    /// each id with its position, for the caller to silence its sound (#1297).
    pub fn apply_pending_destroys(&mut self) -> Vec<(u32, [f32; 3])> {
        let ids = self.cells.scene.borrow_mut().take_pending_destroys();
        if ids.is_empty() {
            return Vec::new();
        }
        let (doomed, order, removed) = {
            let scene = self.cells.scene.borrow();
            let doomed: BTreeSet<u32> = ids.iter().flat_map(|&id| scene.destroy_set(id)).collect();
            let order = teardown_order(&scene, &doomed.iter().copied().collect::<Vec<_>>());
            // World space: a child's sound stops where it was heard, not at its local offset.
            let at = |id: u32| (id, scene.world.world_position(id).to_array());
            let removed = order.iter().map(|&id| at(id)).collect();
            (doomed, order, removed)
        };
        self.dispatch_teardown(&order);
        self.cells
            .scene
            .borrow_mut()
            .despawn_batch(&doomed, Despawn::TornDown);
        self.forget_entities(&doomed.into_iter().collect::<Vec<_>>());
        removed
    }

    /// A scene unload (#432): the same teardown as a destroy, in the same order,
    /// for every entity in `ids` (ascending), and their instances, timers and
    /// coroutines forgotten — but no per-entity removal, because the scene swap that
    /// follows replaces the World.
    pub fn unload_entities(&mut self, ids: &[u32]) {
        let order = teardown_order(&self.cells.scene.borrow(), ids);
        self.dispatch_teardown(&order);
        self.forget_entities(ids);
    }

    /// Fire the teardown of the entities in `order`: `OnDisable` for every
    /// still-enabled instance, then `OnDestroy` for every awoken instance — each pass
    /// over the entities in `order`, an entity's scripts by index, matching Unity's
    /// "OnDisable immediately before OnDestroy".
    fn dispatch_teardown(&self, order: &[u32]) {
        let mut disable_keys = Vec::new();
        let mut destroy_keys = Vec::new();
        for &id in order {
            for (&key, inst) in self.entity_scripts.range((id, 0)..=(id, usize::MAX)) {
                if inst.enabled_last {
                    disable_keys.push(key);
                }
                if inst.awoken {
                    destroy_keys.push(key);
                }
            }
        }
        if disable_keys.is_empty() && destroy_keys.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for &key in &disable_keys {
                self.call_hook(lua, key, ON_DISABLE, key.0);
            }
            for &key in &destroy_keys {
                self.call_hook(lua, key, ON_DESTROY, key.0);
            }
        });
    }

    /// Drop every trace of the entities in `ids` (sorted ascending): their script
    /// instances and load attempts, and their pending timers and coroutines (#444),
    /// so a later entity reusing an id starts clean.
    fn forget_entities(&mut self, ids: &[u32]) {
        let gone = |id: &u32| ids.binary_search(id).is_ok();
        self.cells
            .timers
            .borrow_mut()
            .retain_owners(|owner| !gone(&owner));
        self.entity_scripts.retain(|(eid, _), _| !gone(eid));
        if let Some(lua) = &self.lua {
            for &id in ids {
                let _ = crate::scripting::instances::forget(lua, id);
            }
        }
        self.load_attempted.retain(|(eid, _)| !gone(eid));
    }
}
