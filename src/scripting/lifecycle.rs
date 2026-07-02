use std::collections::{BTreeMap, BTreeSet};

use mlua::Table;

use crate::api;

use super::callbacks::{ON_TRIGGER, ON_TRIGGER_ENTER, ON_TRIGGER_EXIT, START, UPDATE};
use super::manager::ScriptManager;
use crate::physics::TriggerEvents;

impl ScriptManager {
    /// Invokes the Start function on all loaded entity scripts
    pub fn start_scripts(&mut self) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        // Sorted by (entity, script index) so Start order is deterministic
        // (HashMap iteration order varies per run); replays must be byte-identical.
        let mut keys: Vec<(u32, usize)> = self.entity_scripts.keys().copied().collect();
        keys.sort_unstable();

        let ctx = self.make_ctx();
        let _ = lua.scope(|scope| -> mlua::Result<()> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;

            for key in keys {
                let id = key.0;
                let Some(reg) = self.entity_scripts.get(&key) else {
                    continue;
                };
                let Ok(table) = lua.registry_value::<Table>(reg) else {
                    continue;
                };
                let Ok(start_fn) = table.get::<_, mlua::Function>(START) else {
                    continue;
                };
                if let Err(e) = start_fn.call::<_, ()>(id) {
                    self.console.borrow_mut().error(format!(
                        "[Lua Error] {} on entity {} failed: {}",
                        START, id, e
                    ));
                }
            }
            Ok(())
        });
    }

    /// Invokes the Update function on all loaded entity scripts
    pub fn update_scripts(&mut self, delta_time: f32) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        // Collect active-entity keys before the scope so we don't hold a scene
        // borrow across the API registration step.
        let keys = {
            let scene = self.scene.borrow();
            let mut keys: Vec<(u32, usize)> = self
                .entity_scripts
                .keys()
                .copied()
                .filter(|&(id, _)| match scene.get_entity(id) {
                    Some(e) => e.active,
                    None => false,
                })
                .collect();
            // Sorted by (entity, script index) so per-frame Update order is
            // deterministic (HashMap iteration order varies per run). Gameplay now
            // happens inside scripts — e.g. the weapon's Physics.Raycast vs. the enemy's
            // animation Update race in a hit frame — so a stable order is what keeps
            // replays byte-identical.
            keys.sort_unstable();
            keys
        };

        let ctx = self.make_ctx();
        let _ = lua.scope(|scope| -> mlua::Result<()> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;

            for key in &keys {
                let id = key.0;
                let Some(reg) = self.entity_scripts.get(key) else {
                    continue;
                };
                let Ok(table) = lua.registry_value::<Table>(reg) else {
                    continue;
                };
                let Ok(update_fn) = table.get::<_, mlua::Function>(UPDATE) else {
                    continue;
                };
                if let Err(e) = update_fn.call::<_, ()>((id, delta_time)) {
                    self.console.borrow_mut().error(format!(
                        "[Lua Error] {} on entity {} failed: {}",
                        UPDATE, id, e
                    ));
                }
            }
            Ok(())
        });
    }

    /// Invokes the trigger callbacks on scripts of entities involved in trigger
    /// overlaps: `OnTriggerEnter` for this tick's new pairs, then `OnTrigger`
    /// (stay) for every overlapping pair, then `OnTriggerExit` for the pairs
    /// that ended (#310) — a fixed hook order so replays stay byte-identical.
    pub fn dispatch_trigger_events(&mut self, events: TriggerEvents) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        let ctx = self.make_ctx();
        let hooks = [
            (ON_TRIGGER_ENTER, &events.entered),
            (ON_TRIGGER, &events.stayed),
            (ON_TRIGGER_EXIT, &events.exited),
        ];
        let _ = lua.scope(|scope| -> mlua::Result<()> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;

            for (hook, pairs) in hooks {
                for &(id_a, id_b) in pairs {
                    // Notify each side of the overlap, in order: A about B, then B about A.
                    for (id, other) in [(id_a, id_b), (id_b, id_a)] {
                        // An entity may carry many scripts (#83): notify each, in
                        // ascending script-index order so dispatch stays deterministic.
                        let mut indices: Vec<usize> = self
                            .entity_scripts
                            .keys()
                            .filter(|&&(eid, _)| eid == id)
                            .map(|&(_, idx)| idx)
                            .collect();
                        indices.sort_unstable();
                        for idx in indices {
                            let Some(reg) = self.entity_scripts.get(&(id, idx)) else {
                                continue;
                            };
                            let Ok(table) = lua.registry_value::<Table>(reg) else {
                                continue;
                            };
                            let Ok(trigger_fn) = table.get::<_, mlua::Function>(hook) else {
                                continue;
                            };
                            if let Err(e) = trigger_fn.call::<_, ()>((id, other)) {
                                self.console.borrow_mut().error(format!(
                                    "[Lua Error] {} on entity {} failed: {}",
                                    hook, id, e
                                ));
                            }
                        }
                    }
                }
            }
            Ok(())
        });
    }

    /// Evaluate ONE line of Lua against the LIVE runtime and return the result as
    /// a display string. This is the single evaluator behind both the in-editor
    /// console input line and the headless harness, so the two can never drift.
    ///
    /// A REPL line is first tried as an expression (`return <line>`) so values are
    /// echoed back; if that fails to compile it is run as a statement (assignments,
    /// `print(...)`, control flow). Multiple returned values are comma-joined.
    pub fn eval(&self, line: &str) -> Result<String, String> {
        let lua = self
            .lua
            .as_ref()
            .ok_or_else(|| "runtime not initialized (enter Play to evaluate)".to_string())?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        let ctx = self.make_ctx();
        lua.scope(|scope| -> mlua::Result<String> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;

            // Try as an expression first so REPL lines echo their value.
            let values = match lua
                .load(format!("return {}", trimmed))
                .eval::<mlua::MultiValue>()
            {
                Ok(v) => v,
                Err(_) => lua.load(trimmed).eval::<mlua::MultiValue>()?,
            };

            let rendered = values
                .iter()
                .map(value_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            Ok(rendered)
        })
        .map_err(|e| e.to_string())
    }

    /// The live Lua API surface as `namespace → {function names}`.
    ///
    /// Registers the whole `api::` surface into a fresh scope (the namespaces are
    /// scope-tied closures), then walks the Lua globals: every global table that is
    /// not a Lua 5.4 stdlib global contributes its function-valued keys. This is the
    /// ground truth the doc-drift gate checks `docs/scripting-api.md` against, so it
    /// can never silently lie about *what exists* (#280). Empty when the runtime is
    /// not live. Signatures are out of scope — Lua closures are opaque at runtime.
    pub fn api_surface(&self) -> BTreeMap<String, BTreeSet<String>> {
        let Some(lua) = self.lua.as_ref() else {
            return BTreeMap::new();
        };
        let ctx = self.make_ctx();
        let mut surface = BTreeMap::new();
        let _ = lua.scope(|scope| -> mlua::Result<()> {
            api::register(lua, scope, &ctx).map_err(mlua::Error::RuntimeError)?;
            for pair in lua.globals().pairs::<String, mlua::Value>() {
                let (name, value) = pair?;
                let mlua::Value::Table(table) = value else {
                    continue;
                };
                if LUA_STDLIB_GLOBALS.contains(&name.as_str()) {
                    continue;
                }
                let mut fns = BTreeSet::new();
                for entry in table.pairs::<String, mlua::Value>() {
                    let (key, val) = entry?;
                    if matches!(val, mlua::Value::Function(_)) {
                        fns.insert(key);
                    }
                }
                surface.insert(name, fns);
            }
            Ok(())
        });
        surface
    }
}

/// Lua 5.4 standard-library globals — excluded from the API surface walk so the
/// drift gate compares only engine namespaces (capitalized `Debug` is the engine's,
/// not the lowercase stdlib `debug`). The injected `print` override is a function,
/// not a table, so it is filtered out naturally.
const LUA_STDLIB_GLOBALS: &[&str] = &[
    "_G",
    "_VERSION",
    "assert",
    "collectgarbage",
    "dofile",
    "error",
    "getmetatable",
    "ipairs",
    "load",
    "loadfile",
    "next",
    "pairs",
    "pcall",
    "print",
    "rawequal",
    "rawget",
    "rawlen",
    "rawset",
    "select",
    "setmetatable",
    "tonumber",
    "tostring",
    "type",
    "xpcall",
    "require",
    "string",
    "table",
    "math",
    "io",
    "os",
    "coroutine",
    "utf8",
    "package",
    "debug",
];

/// Render a single Lua value for the REPL echo. Mirrors Lua's `tostring`/`print`
/// for the common cases (nil/bool/number/string) and falls back to a typed tag.
fn value_to_string(value: &mlua::Value) -> String {
    match value {
        mlua::Value::Nil => "nil".to_string(),
        mlua::Value::Boolean(b) => b.to_string(),
        mlua::Value::Integer(i) => i.to_string(),
        mlua::Value::Number(n) => n.to_string(),
        mlua::Value::String(s) => s.to_str().map(|s| s.to_string()).unwrap_or_default(),
        mlua::Value::Table(_) => "<table>".to_string(),
        mlua::Value::Function(_) => "<function>".to_string(),
        other => format!("<{}>", other.type_name()),
    }
}
