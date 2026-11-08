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

use mlua::{Lua, Table};

use super::super::callbacks::ON_ANIMATION_EVENT;
use super::super::manager::ScriptManager;
use crate::components::MontageEvent;

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

/// The `{ montage, slot, interrupted }` table a montage event passes.
fn info_table(lua: &Lua, m: &MontageEvent) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("montage", m.montage.as_str())?;
    t.set("slot", m.slot.as_str())?;
    t.set("interrupted", m.interrupted)?;
    Ok(t)
}
