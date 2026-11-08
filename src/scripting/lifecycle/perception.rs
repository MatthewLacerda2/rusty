//! src/scripting/lifecycle/perception.rs — dispatch `OnPerceptionUpdated` (#1900).
//!
//! The perception step and the reported senses (noise, damage) raise updates on
//! each perceiver's memory; the `dispatch_perception_updates` system drains them
//! after the step, before `LateUpdate`, and hands them here. Each calls
//! `OnPerceptionUpdated(id, target, info)` on the perceiver's scripts (Unreal's
//! `OnTargetPerceptionUpdated`), perceivers in id order, each one's updates in
//! the order they were raised, then ascending script index. `target` is `nil`
//! for a noise or hit with no instigator.

use super::super::callbacks::ON_PERCEPTION_UPDATED;
use super::super::manager::ScriptManager;
use crate::api::ai_perception::known_table;
use crate::components::PerceptionUpdate;

impl ScriptManager {
    /// `OnPerceptionUpdated(id, target, info)` for each `(perceiver, update)`, in order.
    pub fn dispatch_perception_updates(&mut self, updates: &[(u32, PerceptionUpdate)]) {
        if updates.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for (id, u) in updates {
                let Ok(info) = known_table(lua, u.target, &u.info, u.forgotten) else {
                    continue;
                };
                for key in self.awoken_keys_for(*id) {
                    let args = (*id, u.target, info.clone());
                    self.call_hook(lua, key, ON_PERCEPTION_UPDATED, args);
                }
            }
        });
    }
}
