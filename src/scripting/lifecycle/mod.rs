//! src/scripting/lifecycle/ — dispatch the lifecycle hooks to entity scripts.
//!
//! Every hook routes through the same two helpers: [`ScriptManager::with_api_scope`]
//! (live-runtime check + API-surface registration) and [`ScriptManager::call_hook`]
//! (resolve table → get function → call → log error) — one dispatch core, so a
//! new callback is a loop, not a fourth copy (#322). Dispatch order is
//! deterministic everywhere: `entity_scripts` is a `BTreeMap`, so iterating it
//! is ascending `(entity id, script index)` — replays must stay byte-identical.
//!
//! The init contract (#322 + #323): [`ScriptManager::init_scripts`] runs at
//! play-enter and again at the head of every tick's script phase. It drains
//! queued script loads (runtime spawns), then dispatches the transition edges in
//! Unity's order — a leaving `OnDisable`, then `Awake`, `OnEnable`, `Start` for
//! entering instances — so an entity's first active tick fires
//! `Awake → OnEnable → Start` before any `Update`. Enable/disable edges are
//! detected by diffing each instance's `enabled_last` flag against the entity's
//! current `active` state, so each edge fires exactly once. Every gameplay hook
//! obeys ONE active gate ([`ScriptManager::entity_active`]): a disabled entity
//! receives no callbacks at all — `Update`, `LateUpdate` and the trigger hooks
//! alike — which is precisely what `OnDisable` announces.
//!
//! The transition-specific dispatch — the `OnDisable` falling-edge sweep and the
//! deferred-destroy drain (`OnDisable`→`OnDestroy`) — lives in the `transitions`
//! submodule, the UI callbacks (#420) in `ui`, the trigger and collision callbacks
//! (#310, #448) in `physics`, `OnAnimationEvent` (#459) in `animation`; this file holds the shared dispatch core and the
//! per-frame hooks.

mod animation;
mod physics;
mod transitions;
mod ui;

use mlua::{Lua, Table};

use super::callbacks::{AWAKE, LATE_UPDATE, ON_ENABLE, START, UPDATE};
use super::manager::{ScriptInstance, ScriptManager};
use crate::api;

impl ScriptManager {
    /// Run `body` with the full API surface registered into a live-runtime
    /// scope — the shared ceremony around every hook dispatch. No-op when the
    /// runtime is not live.
    pub(super) fn with_api_scope(&self, body: impl FnOnce(&Lua)) {
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

    /// The single active gate every gameplay hook obeys: `Some(true)` when the
    /// entity exists and is active, `Some(false)` when it exists but is inactive,
    /// `None` when it is gone. Enable/disable edge detection and the
    /// `Update`/`LateUpdate`/trigger dispatch all read it, so no hook can drift
    /// from the rule "a disabled entity receives no callbacks".
    pub(super) fn entity_active(&self, id: u32) -> Option<bool> {
        let scene = self.scene.borrow();
        scene.world.contains(id).then(|| scene.world.is_active(id))
    }

    /// Ascending keys of instances passing `pred` whose owning entity is active —
    /// the dispatch-eligible set, collected before the scope so no scene borrow is
    /// held across dispatch (scripts re-borrow the scene).
    fn eligible_keys(&self, pred: impl Fn(&ScriptInstance) -> bool) -> Vec<(u32, usize)> {
        self.entity_scripts
            .iter()
            .filter(|(&(id, _), inst)| pred(inst) && self.entity_active(id) == Some(true))
            .map(|(&key, _)| key)
            .collect()
    }

    /// Call the single-arg hook `hook(id)` on `keys`, in the given order, inside
    /// one API scope. The shared body behind every `(id)`-signature dispatch
    /// (Awake/OnEnable/Start/OnDisable/OnDestroy); marking the instances is the
    /// caller's job so each phase records its own edge exactly once.
    fn dispatch_keys(&self, keys: &[(u32, usize)], hook: &str) {
        if keys.is_empty() {
            return;
        }
        self.with_api_scope(|lua| {
            for &key in keys {
                self.call_hook(lua, key, hook, key.0);
            }
        });
    }

    /// The init phase at the head of the script phase (and at play-enter): drain
    /// queued script loads, then dispatch the transition edges in Unity's order —
    /// leaving instances' `OnDisable` first, then `Awake`, `OnEnable`, `Start` for
    /// entering ones. An entity's first active tick therefore fires
    /// `Awake → OnEnable → Start`, all before any `Update` of the tick (#322/#323).
    pub fn init_scripts(&mut self) {
        self.load_new_scripts();
        // Falling edges: an awoken, currently-enabled instance whose entity went
        // inactive since the last sweep gets `OnDisable`, then clears the flag.
        self.dispatch_disabled();
        // Rising path, in order. `Awake` marks `awoken`; `OnEnable` then sees the
        // fresh instance (awoken && !enabled_last) and slots between Awake and Start.
        self.dispatch_pending(|i| !i.awoken, AWAKE, |i| i.awoken = true);
        self.dispatch_pending(
            |i| i.awoken && !i.enabled_last,
            ON_ENABLE,
            |i| i.enabled_last = true,
        );
        self.dispatch_pending(|i| i.awoken && !i.started, START, |i| i.started = true);
    }

    /// One rising-edge sub-phase: call `hook(id)` on every active instance passing
    /// `pred`, in ascending key order, then `mark` each so it never re-fires. The
    /// eligible set is re-read per phase, so an `Awake` that deactivates an entity
    /// defers that entity's `OnEnable`/`Start`.
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

    /// Ascending script-slot keys of entity `id`'s awoken instances — empty when
    /// the entity is inactive or gone, so the trigger hooks share the same active
    /// gate as every other gameplay callback (#323).
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
