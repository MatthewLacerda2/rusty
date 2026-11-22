use mlua::{Function, Lua};

use super::super::super::manager::ScriptManager;

impl ScriptManager {
    pub(in super::super) fn step_tween(&self, lua: &Lua, handle: u64, owner: u32) {
        let t = self.cells.timers.borrow().overshoot(handle);
        let done = {
            let mut timers = self.cells.timers.borrow_mut();
            let mut scene = self.cells.scene.borrow_mut();
            let Some(tween) = timers.tween_mut(handle) else {
                return;
            };
            let Some(from) = tween.from.or_else(|| tween.property.get(&scene, owner)) else {
                timers.cancel(handle);
                return;
            };
            tween.from = Some(from);
            let (value, done) = tween.sample(from, t);
            if !tween.property.set(&mut scene, owner, value) {
                timers.cancel(handle);
                return;
            }
            done
        };
        if !done {
            return;
        }
        let finished = self.cells.timers.borrow_mut().take_tween(handle);
        let Some(key) = finished.and_then(|t| t.on_complete) else {
            return;
        };
        let called = lua
            .registry_value::<Function>(&key)
            .and_then(|f| f.call::<()>(owner));
        if let Err(e) = called {
            self.log_error(owner, "Tween on_complete", &e.to_string());
        }
    }
}
