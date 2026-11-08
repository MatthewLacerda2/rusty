//! src/scripting/lifecycle/animation.rs — dispatch `OnAnimationEvent` (#459).
//!
//! The `animate` system queues the events each animator's playheads crossed this
//! tick; the `dispatch_animation_events` system hands them here once the
//! post-animation pose is settled (after ragdolls, before `LateUpdate`). Each
//! event calls `OnAnimationEvent(id, name, info)` on the scripts of the entity
//! whose Animator played it — Unity sends an `AnimationEvent` to the Animator's
//! GameObject — in queue order, then ascending script index. `info` is a
//! montage's engine event's `{ montage, slot, interrupted }` (#1894) and `nil`
//! for every other event, so a two-argument handler is unchanged.
//!
//! The graph-state callbacks (#1901) go the same way, just before the events:
//! `OnStateEnter` / `OnStateUpdate` / `OnStateExit(id, info)`, `info` Unity's
//! `AnimatorStateInfo` as a table. `OnStateUpdate` fires every step for every
//! active state, so its table is built only when a script defines it.

use mlua::{Lua, Table};

use super::super::callbacks::{ON_ANIMATION_EVENT, ON_STATE_ENTER, ON_STATE_EXIT, ON_STATE_UPDATE};
use super::super::manager::ScriptManager;
use crate::components::{MontageEvent, StateCallback, StateEvent, StateInfo};

impl ScriptManager {
    /// `OnAnimationEvent(id, name, info)` for each queued `(entity, event name,
    /// whose montage)`, in order.
    pub fn dispatch_animation_events(&mut self, events: &[(u32, String, Option<MontageEvent>)]) {
        if events.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for (id, name, montage) in events {
                let info = montage.as_ref().and_then(|m| info_table(lua, m).ok());
                for key in self.awoken_keys_for(*id) {
                    let args = (*id, name.as_str(), info.clone());
                    self.call_hook(lua, key, ON_ANIMATION_EVENT, args);
                }
            }
        });
    }
}

impl ScriptManager {
    /// `OnStateEnter` / `OnStateUpdate` / `OnStateExit(id, info)` for each
    /// queued `(entity, callback)`, in order.
    pub fn dispatch_state_callbacks(&mut self, callbacks: &[(u32, StateCallback)]) {
        if callbacks.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for (id, callback) in callbacks {
                let hook = match callback.event {
                    StateEvent::Enter => ON_STATE_ENTER,
                    StateEvent::Update => ON_STATE_UPDATE,
                    StateEvent::Exit => ON_STATE_EXIT,
                };
                let keys = self.awoken_keys_for(*id).into_iter();
                let keys: Vec<_> = keys.filter(|k| self.defines(lua, *k, hook)).collect();
                if keys.is_empty() {
                    continue;
                }
                let Ok(info) = state_table(lua, &callback.info) else {
                    continue;
                };
                for key in keys {
                    self.call_hook(lua, key, hook, (*id, info.clone()));
                }
            }
        });
    }
}

/// Unity's `AnimatorStateInfo` as the state callbacks pass it.
fn state_table(lua: &Lua, info: &StateInfo) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("layer", info.layer)?;
    t.set("layer_name", info.layer_name.as_str())?;
    t.set("state", info.state.as_str())?;
    t.set("tag", info.tag.as_deref())?;
    t.set("time", info.time)?;
    t.set("normalized_time", info.normalized_time)?;
    t.set("length", info.length)?;
    Ok(t)
}

/// The `{ montage, slot, interrupted }` table a montage event passes.
fn info_table(lua: &Lua, m: &MontageEvent) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("montage", m.montage.as_str())?;
    t.set("slot", m.slot.as_str())?;
    t.set("interrupted", m.interrupted)?;
    Ok(t)
}
