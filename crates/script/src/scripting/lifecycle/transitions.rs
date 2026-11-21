use std::cmp::Reverse;
use std::collections::BTreeSet;

use super::super::callbacks::{ON_DESTROY, ON_DISABLE};
use super::super::manager::{ScriptInstance, ScriptManager};
use rusty_scene::{Despawn, Scene};

fn teardown_order(scene: &Scene, ids: &[u32]) -> Vec<u32> {
    let mut order = ids.to_vec();
    order.sort_by_cached_key(|&id| Reverse(scene.world.ancestors(id).count()));
    order
}

impl ScriptManager {
    fn disabled_keys(&self, pred: impl Fn(&ScriptInstance) -> bool) -> Vec<(u32, usize)> {
        self.entity_scripts
            .iter()
            .filter(|(&(id, _), inst)| pred(inst) && self.entity_active(id) == Some(false))
            .map(|(&key, _)| key)
            .collect()
    }

    pub(super) fn dispatch_disabled(&mut self) {
        let keys = self.disabled_keys(|i| i.awoken && i.enabled_last);
        self.dispatch_keys(&keys, ON_DISABLE);
        for &(id, _) in &keys {
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

    pub fn apply_pending_destroys(&mut self) -> Vec<(u32, [f32; 3])> {
        let ids = self.cells.scene.borrow_mut().take_pending_destroys();
        if ids.is_empty() {
            return Vec::new();
        }
        let (doomed, order, removed) = {
            let scene = self.cells.scene.borrow();
            let doomed: BTreeSet<u32> = ids.iter().flat_map(|&id| scene.destroy_set(id)).collect();
            let order = teardown_order(&scene, &doomed.iter().copied().collect::<Vec<_>>());
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

    pub fn unload_entities(&mut self, ids: &[u32]) {
        let order = teardown_order(&self.cells.scene.borrow(), ids);
        self.dispatch_teardown(&order);
        self.forget_entities(ids);
    }

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
        self.with_lua(|lua| {
            for &key in &disable_keys {
                self.call_hook(lua, key, ON_DISABLE, key.0);
            }
            for &key in &destroy_keys {
                self.call_hook(lua, key, ON_DESTROY, key.0);
            }
        });
    }

    fn forget_entities(&mut self, ids: &[u32]) {
        let gone = |id: &u32| ids.binary_search(id).is_ok();
        self.cells
            .timers
            .borrow_mut()
            .retain_owners(|owner| !gone(&owner));
        self.cells
            .queries
            .borrow_mut()
            .retain_owners(|owner| !gone(&owner));
        self.entity_scripts.retain(|(eid, _), _| !gone(eid));
        if let Some(lua) = &self.lua {
            for &id in ids {
                let _ = crate::scripting::instances::forget(lua, id);
            }
        }
        let requeued = &mut self.requeued;
        self.load_attempted.retain(|&(eid, _)| {
            let keep = !gone(&eid);
            if !keep {
                requeued.insert(eid);
            }
            keep
        });
    }
}
