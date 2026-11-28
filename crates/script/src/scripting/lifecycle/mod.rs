mod animation;
mod controller;
mod perception;
mod physics;
mod transitions;
mod ui;

pub use controller::dispatch_controller_hits;

use mlua::{Lua, Table};

use super::callbacks::{AWAKE, LATE_UPDATE, ON_ENABLE, ON_ORIGIN_SHIFT, START, UPDATE};
use super::manager::{ScriptInstance, ScriptManager};

impl ScriptManager {
    pub(super) fn with_lua(&self, body: impl FnOnce(&Lua)) {
        if let Some(lua) = &self.lua {
            body(lua);
        }
    }

    fn hook(&self, lua: &Lua, key: (u32, usize), hook: &str) -> Option<mlua::Function> {
        let inst = self.entity_scripts.get(&key)?;
        let table = lua.registry_value::<Table>(&inst.table).ok()?;
        table.get::<mlua::Function>(hook).ok()
    }

    fn call_hook(&self, lua: &Lua, key: (u32, usize), hook: &str, args: impl mlua::IntoLuaMulti) {
        if let Some(func) = self.hook(lua, key, hook) {
            self.call(&func, key, hook, args);
        }
    }

    fn call(
        &self,
        func: &mlua::Function,
        key: (u32, usize),
        hook: &str,
        args: impl mlua::IntoLuaMulti,
    ) {
        if let Err(e) = func.call::<()>(args) {
            self.cells.console.borrow_mut().error(format!(
                "[Lua Error] {} on entity {} failed: {}",
                hook, key.0, e
            ));
        }
    }

    pub(super) fn entity_active(&self, id: u32) -> Option<bool> {
        let scene = self.cells.scene.borrow();
        scene
            .world
            .contains(id)
            .then(|| scene.world.is_active_in_hierarchy(id))
    }

    /// Collected before dispatch, since the hooks re-borrow the scene.
    fn eligible_keys(&self, pred: impl Fn(&ScriptInstance) -> bool) -> Vec<(u32, usize)> {
        self.entity_scripts
            .iter()
            .filter(|(&(id, _), inst)| pred(inst) && self.entity_active(id) == Some(true))
            .map(|(&key, _)| key)
            .collect()
    }

    fn dispatch_keys(&self, keys: &[(u32, usize)], hook: &str) {
        if keys.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for &key in keys {
                self.call_hook(lua, key, hook, key.0);
            }
        });
    }

    pub fn init_scripts(&mut self) {
        self.load_new_scripts();
        self.dispatch_disabled();
        self.dispatch_pending(|i| !i.awoken, AWAKE, |i| i.awoken = true);
        self.dispatch_pending(
            |i| i.awoken && !i.enabled_last,
            ON_ENABLE,
            |i| i.enabled_last = true,
        );
        self.dispatch_pending(|i| i.awoken && !i.started, START, |i| i.started = true);
    }

    /// Eligibility is re-read per phase, so an `Awake` that deactivates its entity defers
    /// `OnEnable` and `Start`.
    fn dispatch_pending(
        &mut self,
        pred: impl Fn(&ScriptInstance) -> bool,
        hook: &str,
        mark: impl Fn(&mut ScriptInstance),
    ) {
        let keys = self.eligible_keys(pred);
        self.dispatch_keys(&keys, hook);
        for key in keys {
            if let Some(inst) = self.entity_scripts.get_mut(&key) {
                mark(inst);
            }
        }
    }

    pub fn update_scripts(&mut self, delta_time: f32) {
        let keys = self.eligible_keys(|inst| inst.started);
        if keys.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, UPDATE, (key.0, delta_time));
            }
        });
    }

    pub fn late_update_scripts(&mut self, delta_time: f32) {
        let keys = self.eligible_keys(|inst| inst.started);
        if keys.is_empty() {
            return;
        }
        self.with_lua(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, LATE_UPDATE, (key.0, delta_time));
            }
        });
    }

    pub fn dispatch_origin_shift(&mut self, by: glam::Vec3) {
        self.cells.timers.borrow_mut().shift_origin(by);
        self.cells.queries.borrow_mut().shift_origin(by);
        let keys = self.eligible_keys(|inst| inst.awoken);
        self.with_lua(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, ON_ORIGIN_SHIFT, (key.0, by.x, by.y, by.z));
            }
        });
    }

    fn awoken_keys_for(&self, id: u32) -> Vec<(u32, usize)> {
        if self.entity_active(id) != Some(true) {
            return Vec::new();
        }
        self.entity_scripts
            .range((id, 0)..=(id, usize::MAX))
            .filter(|(_, inst)| inst.awoken)
            .map(|(&key, _)| key)
            .collect()
    }
}
