//! src/scripting/lifecycle/animation.rs — dispatch `OnAnimationEvent` (#459).
//!
//! The `animate` system queues the events each animator's playheads crossed this
//! tick; the `dispatch_animation_events` system hands them here once the
//! post-animation pose is settled (after ragdolls, before `LateUpdate`). Each
//! event calls `OnAnimationEvent(id, name)` on the scripts of the entity whose
//! Animator played it — Unity sends an `AnimationEvent` to the Animator's
//! GameObject — in queue order, then ascending script index.

use super::super::callbacks::ON_ANIMATION_EVENT;
use super::super::manager::ScriptManager;

impl ScriptManager {
    /// `OnAnimationEvent(id, name)` for each queued `(entity, event name)`, in
    /// order.
    pub fn dispatch_animation_events(&mut self, events: &[(u32, String)]) {
        if events.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for (id, name) in events {
                for key in self.awoken_keys_for(*id) {
                    self.call_hook(lua, key, ON_ANIMATION_EVENT, (*id, name.as_str()));
                }
            }
        });
    }
}
