use mlua::{Lua, Table};

use super::super::callbacks::{ON_ANIMATION_EVENT, ON_STATE_ENTER, ON_STATE_EXIT, ON_STATE_UPDATE};
use super::super::manager::ScriptManager;
use rusty_components::{MontageEvent, StateCallback, StateEvent, StateInfo};

impl ScriptManager {
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

const STATE_HOOKS: [&str; 3] = [ON_STATE_ENTER, ON_STATE_UPDATE, ON_STATE_EXIT];

impl ScriptManager {
    /// The animation step diffs `id`'s states only while this holds, so a script woken later
    /// hears every active state enter.
    pub fn hears_states(&self, id: u32) -> bool {
        let Some(lua) = &self.lua else {
            return false;
        };
        self.state_keys(id)
            .any(|key| STATE_HOOKS.iter().any(|h| self.hook(lua, key, h).is_some()))
    }

    fn state_keys(&self, id: u32) -> impl Iterator<Item = (u32, usize)> + '_ {
        let enabled = |key: &(u32, usize)| self.entity_scripts[key].enabled_last;
        self.awoken_keys_for(id).into_iter().filter(enabled)
    }

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
                let funcs: Vec<_> = self
                    .state_keys(*id)
                    .filter_map(|key| Some((key, self.hook(lua, key, hook)?)))
                    .collect();
                if funcs.is_empty() {
                    continue;
                }
                let Ok(info) = state_table(lua, &callback.info) else {
                    continue;
                };
                for (key, func) in funcs {
                    self.call(&func, key, hook, (*id, info.clone()));
                }
            }
        });
    }
}

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

fn info_table(lua: &Lua, m: &MontageEvent) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("montage", m.montage.as_str())?;
    t.set("slot", m.slot.as_str())?;
    t.set("interrupted", m.interrupted)?;
    Ok(t)
}
