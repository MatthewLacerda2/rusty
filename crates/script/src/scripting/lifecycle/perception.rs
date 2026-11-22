use super::super::callbacks::ON_PERCEPTION_UPDATED;
use super::super::manager::ScriptManager;
use crate::api::ai_perception::known_table;
use rusty_components::PerceptionUpdate;

impl ScriptManager {
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
