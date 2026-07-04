//! src/scripting/lifecycle.rs — dispatch the lifecycle hooks to entity scripts.
//!
//! Every hook routes through the same two helpers: [`ScriptManager::with_api_scope`]
//! (live-runtime check + API-surface registration) and [`ScriptManager::call_hook`]
//! (resolve table → get function → call → log error) — one dispatch core, so a
//! new callback is a loop, not a fourth copy (#322). Dispatch order is
//! deterministic everywhere: `entity_scripts` is a `BTreeMap`, so iterating it
//! is ascending `(entity id, script index)` — replays must stay byte-identical.
//!
//! The init contract (#322): [`ScriptManager::init_scripts`] runs at play-enter
//! and again at the head of every tick's script phase. It drains queued script
//! loads (runtime spawns), then fires every pending `Awake`, then every pending
//! `Start` — two phases, so a `Start` can rely on state any other script set up
//! in `Awake`, matching Unity. Both hooks are gated on the owning entity being
//! active — that is what defers a disabled-at-load entity's init to its first
//! active tick — and each fires exactly once per instance (the
//! `ScriptInstance` flags).

use mlua::{Lua, Table};

use crate::api;
use crate::physics::TriggerEvents;

use super::callbacks::{
    AWAKE, LATE_UPDATE, ON_TRIGGER, ON_TRIGGER_ENTER, ON_TRIGGER_EXIT, START, UPDATE,
};
use super::manager::{ScriptInstance, ScriptManager};

impl ScriptManager {
    /// Run `body` with the full API surface registered into a live-runtime
    /// scope — the shared ceremony around every hook dispatch. No-op when the
    /// runtime is not live.
    fn with_api_scope(&self, body: impl FnOnce(&Lua)) {
        let Some(lua) = &self.lua else { return };
        let ctx = self.make_ctx();
        let _ = lua.scope(|scope| -> mlua::Result<()> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            body(lua);
            Ok(())
        });
    }

    /// The generic hook call every dispatch loop shares: resolve the instance's
    /// lifecycle table, look up `hook`, call it with `args`, log a Lua error to
    /// the console. A script that doesn't define `hook` is silently skipped
    /// (every callback is optional).
    fn call_hook<'lua>(
        &self,
        lua: &'lua Lua,
        key: (u32, usize),
        hook: &str,
        args: impl mlua::IntoLuaMulti<'lua>,
    ) {
        let Some(inst) = self.entity_scripts.get(&key) else {
            return;
        };
        let Ok(table) = lua.registry_value::<Table>(&inst.table) else {
            return;
        };
        let Ok(func) = table.get::<_, mlua::Function>(hook) else {
            return;
        };
        if let Err(e) = func.call::<_, ()>(args) {
            self.console.borrow_mut().error(format!(
                "[Lua Error] {} on entity {} failed: {}",
                hook, key.0, e
            ));
        }
    }

    /// Ascending keys of instances passing `pred` whose owning entity exists
    /// and is active — the dispatch-eligible set, collected before the scope so
    /// no scene borrow is held across dispatch (scripts re-borrow the scene).
    fn eligible_keys(&self, pred: impl Fn(&ScriptInstance) -> bool) -> Vec<(u32, usize)> {
        let scene = self.scene.borrow();
        self.entity_scripts
            .iter()
            .filter(|(&(id, _), inst)| pred(inst) && scene.get_entity(id).is_some_and(|e| e.active))
            .map(|(&key, _)| key)
            .collect()
    }

    /// The init phase at the head of the script phase (and at play-enter):
    /// drain queued script loads, then fire all pending `Awake`s, then all
    /// pending `Start`s — before any `Update` of the tick (#322).
    pub fn init_scripts(&mut self) {
        self.load_new_scripts();
        self.dispatch_pending(|i| !i.awoken, AWAKE, |i| i.awoken = true);
        self.dispatch_pending(|i| i.awoken && !i.started, START, |i| i.started = true);
    }

    /// One init sub-phase: call `hook(id)` on every eligible instance passing
    /// `pred`, in ascending key order, then `mark` each so it never re-fires.
    /// The eligible set is re-read per phase, so an `Awake` that deactivates an
    /// entity defers that entity's `Start`.
    fn dispatch_pending(
        &mut self,
        pred: impl Fn(&ScriptInstance) -> bool,
        hook: &str,
        mark: impl Fn(&mut ScriptInstance),
    ) {
        let keys = self.eligible_keys(pred);
        if keys.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, hook, key.0);
            }
        });
        for key in keys {
            if let Some(inst) = self.entity_scripts.get_mut(&key) {
                mark(inst);
            }
        }
    }

    /// Invokes `Update(id, dt)` on every started instance whose entity is
    /// active. Gated on `started` so `Start` always precedes the first `Update`
    /// — `init_scripts` runs at the head of the same script phase.
    pub fn update_scripts(&mut self, delta_time: f32) {
        let keys = self.eligible_keys(|inst| inst.started);
        if keys.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, UPDATE, (key.0, delta_time));
            }
        });
    }

    /// Invokes `LateUpdate(id, dt)` on every started instance whose entity is
    /// active — the post-physics twin of [`update_scripts`](Self::update_scripts)
    /// (#324). Its play-mode system is registered after physics/animation/particles
    /// and before `advance_frame`, so a follow-cam / look-at / aim script reads
    /// *this* tick's resolved transforms, not last tick's. Same eligible set,
    /// same ascending `(entity, script index)` order, and the same scaled `dt`
    /// the tick handed `Update` — it rides the shared dispatch core, so it is one
    /// more loop, not a new copy of it.
    pub fn late_update_scripts(&mut self, delta_time: f32) {
        let keys = self.eligible_keys(|inst| inst.started);
        if keys.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for &key in &keys {
                self.call_hook(lua, key, LATE_UPDATE, (key.0, delta_time));
            }
        });
    }

    /// Invokes the trigger callbacks on scripts of entities involved in trigger
    /// overlaps: `OnTriggerEnter` for this tick's new pairs, then `OnTrigger`
    /// (stay) for every overlapping pair, then `OnTriggerExit` for the pairs
    /// that ended (#310) — a fixed hook order so replays stay byte-identical.
    /// Only awoken instances are notified: `Awake` is always an instance's
    /// first callback.
    pub fn dispatch_trigger_events(&mut self, events: TriggerEvents) {
        let hooks = [
            (ON_TRIGGER_ENTER, &events.entered),
            (ON_TRIGGER, &events.stayed),
            (ON_TRIGGER_EXIT, &events.exited),
        ];
        self.with_api_scope(|lua| {
            for (hook, pairs) in hooks {
                for &(id_a, id_b) in pairs {
                    // Notify each side of the overlap, in order: A about B, then B about A.
                    for (id, other) in [(id_a, id_b), (id_b, id_a)] {
                        // An entity may carry many scripts (#83): notify each, in
                        // ascending script-index order so dispatch stays deterministic.
                        for key in self.awoken_keys_for(id) {
                            self.call_hook(lua, key, hook, (id, other));
                        }
                    }
                }
            }
        });
    }

    /// Ascending script-slot keys of entity `id`'s awoken instances.
    fn awoken_keys_for(&self, id: u32) -> Vec<(u32, usize)> {
        self.entity_scripts
            .range((id, 0)..=(id, usize::MAX))
            .filter(|(_, inst)| inst.awoken)
            .map(|(&key, _)| key)
            .collect()
    }
}
