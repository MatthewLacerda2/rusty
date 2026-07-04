//! src/scripting/lifecycle/transitions.rs — the enable/disable/destroy dispatch.
//!
//! The #323 transition callbacks that don't ride the plain active-gated init
//! path in `mod.rs`:
//!   - `OnDisable` on the falling edge — an entity that was active is now
//!     inactive. Detected by a diff against the previous tick, so it needs the
//!     *inactive* set (`disabled_keys`), the mirror of `mod.rs`'s `eligible_keys`.
//!   - `OnDestroy` (preceded by `OnDisable` for still-active instances) on the
//!     deferred-destroy drain: `Scene.DestroyEntity` queued ids this tick, and
//!     `apply_pending_destroys` fires teardown then removes the entities.
//!
//! Both reuse the shared dispatch core (`dispatch_keys`, `call_hook`,
//! `entity_active`) so they stay loops, not copies.

use super::super::callbacks::{ON_DESTROY, ON_DISABLE};
use super::super::manager::{ScriptInstance, ScriptManager};

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
        for key in keys {
            if let Some(inst) = self.entity_scripts.get_mut(&key) {
                inst.enabled_last = false;
            }
        }
    }

    /// Drain the scene's deferred-destroy queue (filled by play-mode
    /// `Scene.DestroyEntity`) and dispatch each doomed entity's teardown (#323):
    /// `OnDisable` for every still-enabled instance, then `OnDestroy` for every
    /// awoken instance — the whole batch in ascending `(entity, script index)`
    /// order, matching Unity's "OnDisable immediately before OnDestroy". The
    /// entities stay live through dispatch (so an `OnDestroy` can read its own
    /// transform), then are removed along with their script instances. Cascading
    /// destroys requested from within an `OnDestroy` fall to the next tick's drain,
    /// since the queue was taken up front.
    pub fn apply_pending_destroys(&mut self) {
        let ids = self.scene.borrow_mut().take_pending_destroys();
        if ids.is_empty() {
            return;
        }
        let mut disable_keys = Vec::new();
        let mut destroy_keys = Vec::new();
        for (&key, inst) in &self.entity_scripts {
            if ids.contains(&key.0) {
                if inst.enabled_last {
                    disable_keys.push(key);
                }
                if inst.awoken {
                    destroy_keys.push(key);
                }
            }
        }
        self.with_api_scope(|lua| {
            for &key in &disable_keys {
                self.call_hook(lua, key, ON_DISABLE, key.0);
            }
            for &key in &destroy_keys {
                self.call_hook(lua, key, ON_DESTROY, key.0);
            }
        });
        for &id in &ids {
            self.scene.borrow_mut().destroy_entity(id);
        }
        self.entity_scripts
            .retain(|&(eid, _), _| !ids.contains(&eid));
        self.load_attempted.retain(|&(eid, _)| !ids.contains(&eid));
    }
}
