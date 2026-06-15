use std::cell::RefCell;

use mlua::{Lua, Scope, Table};

use crate::api::{self, ApiCtx};

use super::manager::{ScriptCtx, ScriptManager};

/// Open ONE `lua.scope` around `body`, with the whole engine API (plus `print`)
/// registered as scoped callbacks over the borrowed engine state in `ctx`.
///
/// Each `&mut T` in `ctx` is wrapped in a transient stack `RefCell` that lives for
/// the duration of this call (longer than the scope), so several re-entrant
/// bindings can share `&mut Scene` &c. without any heap-shared interior
/// mutability (#57). `body` runs the lifecycle/REPL calls; the `console` cell is
/// reachable inside `body` for error logging via [`ScriptManager`]'s helpers.
pub(super) fn with_api_scope<R>(lua: &Lua, ctx: &mut ScriptCtx, body: impl FnOnce(&Lua) -> R) -> R {
    let scene = RefCell::new(&mut *ctx.scene);
    let input = RefCell::new(&mut *ctx.input);
    let nav = RefCell::new(&mut *ctx.nav);
    let console = RefCell::new(&mut *ctx.console);
    let camera = RefCell::new(&mut *ctx.camera);
    let time = RefCell::new(&mut *ctx.time);
    let physics = RefCell::new(&mut *ctx.physics);

    let api_ctx = ApiCtx {
        scene: &scene,
        input: &input,
        nav: &nav,
        camera: &camera,
        time: &time,
        physics: &physics,
        console: &console,
    };

    // `lua.scope` returns `mlua::Result<R>`; ours never fails (the inner closure
    // does its own error handling), so unwrap the always-Ok wrapper.
    lua.scope(|scope| {
        register_console_sinks(scope, lua, &console).map_err(mlua::Error::external)?;
        api::register(scope, lua, &api_ctx).map_err(mlua::Error::external)?;
        Ok(body(lua))
    })
    .expect("script-run scope setup is infallible")
}

/// The global key the scoped error sink is registered under, used by [`log_error`]
/// to route lifecycle errors into the same borrowed console the bindings write to.
const ERROR_SINK: &str = "__rusty_error";

/// Register `print` (info) and the error sink (error level) as scoped callbacks
/// over the borrowed console, so both share the one console cell the rest of the
/// surface binds against — no second Rust-side borrow of the console is needed.
fn register_console_sinks<'a, 'scope>(
    scope: &Scope<'a, 'scope>,
    lua: &'a Lua,
    console: &'scope RefCell<&'scope mut super::ConsoleLogs>,
) -> Result<(), String> {
    let print_fn = scope
        .create_function(move |_, msg: String| {
            console.borrow_mut().info(msg);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    lua.globals()
        .set("print", print_fn)
        .map_err(|e| e.to_string())?;

    let error_fn = scope
        .create_function(move |_, msg: String| {
            console.borrow_mut().error(msg);
            Ok(())
        })
        .map_err(|e| e.to_string())?;
    lua.globals()
        .set(ERROR_SINK, error_fn)
        .map_err(|e| e.to_string())
}

impl ScriptManager {
    /// Invokes the Start function on all loaded entity scripts.
    pub fn start_scripts(&mut self, ctx: &mut ScriptCtx) {
        let Some(lua) = self.lua.as_ref() else {
            return;
        };

        // Sorted so script Start order is deterministic (HashMap iteration order
        // varies per run); replays must be byte-identical.
        let mut ids: Vec<u32> = self.entity_scripts.keys().copied().collect();
        ids.sort_unstable();

        let scripts = &self.entity_scripts;
        with_api_scope(lua, ctx, |lua| {
            for id in ids {
                let Some(key) = scripts.get(&id) else {
                    continue;
                };
                let Ok(table) = lua.registry_value::<Table>(key) else {
                    continue;
                };
                let Ok(start_fn) = table.get::<_, mlua::Function>("Start") else {
                    continue;
                };
                if let Err(e) = start_fn.call::<_, ()>(id) {
                    log_error(
                        lua,
                        format!("[Lua Error] Start on entity {} failed: {}", id, e),
                    );
                }
            }
        });
    }

    /// Invokes the Update function on all loaded entity scripts.
    pub fn update_scripts(&mut self, ctx: &mut ScriptCtx, delta_time: f32) {
        let Some(lua) = self.lua.as_ref() else {
            return;
        };

        // Filter active entities in the scene that have scripts.
        let mut ids: Vec<u32> = self
            .entity_scripts
            .keys()
            .copied()
            .filter(|&id| ctx.scene.get_entity(id).map(|e| e.active).unwrap_or(false))
            .collect();
        // Sorted so per-frame Update order is deterministic (HashMap iteration
        // order varies per run). Gameplay now happens inside scripts — e.g. the
        // weapon's Physics.Shoot vs. the enemy's animation Update race in the
        // kill frame — so a stable order is what keeps replays byte-identical.
        ids.sort_unstable();

        let scripts = &self.entity_scripts;
        with_api_scope(lua, ctx, |lua| {
            for id in ids {
                let Some(key) = scripts.get(&id) else {
                    continue;
                };
                let Ok(table) = lua.registry_value::<Table>(key) else {
                    continue;
                };
                let Ok(update_fn) = table.get::<_, mlua::Function>("Update") else {
                    continue;
                };
                if let Err(e) = update_fn.call::<_, ()>((id, delta_time)) {
                    log_error(
                        lua,
                        format!("[Lua Error] Update on entity {} failed: {}", id, e),
                    );
                }
            }
        });
    }

    /// Invokes `OnTrigger` on scripts of entities involved in a trigger overlap.
    pub fn dispatch_trigger_events(&mut self, ctx: &mut ScriptCtx, events: Vec<(u32, u32)>) {
        let Some(lua) = self.lua.as_ref() else {
            return;
        };

        let scripts = &self.entity_scripts;
        with_api_scope(lua, ctx, |lua| {
            for (id_a, id_b) in events {
                // Notify each side of the overlap, in order: A about B, then B about A.
                for (id, other) in [(id_a, id_b), (id_b, id_a)] {
                    let Some(key) = scripts.get(&id) else {
                        continue;
                    };
                    let Ok(table) = lua.registry_value::<Table>(key) else {
                        continue;
                    };
                    let Ok(trigger_fn) = table.get::<_, mlua::Function>("OnTrigger") else {
                        continue;
                    };
                    if let Err(e) = trigger_fn.call::<_, ()>((id, other)) {
                        log_error(
                            lua,
                            format!("[Lua Error] OnTrigger on entity {} failed: {}", id, e),
                        );
                    }
                }
            }
        });
    }

    /// Evaluate ONE line of Lua against the LIVE runtime and return the result as
    /// a display string. This is the single evaluator behind both the in-editor
    /// console input line and the headless harness, so the two can never drift.
    ///
    /// A REPL line is first tried as an expression (`return <line>`) so values are
    /// echoed back; if that fails to compile it is run as a statement (assignments,
    /// `print(...)`, control flow). Multiple returned values are comma-joined.
    pub fn eval(&self, ctx: &mut ScriptCtx, line: &str) -> Result<String, String> {
        let lua = self
            .lua
            .as_ref()
            .ok_or_else(|| "runtime not initialized (enter Play to evaluate)".to_string())?;

        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Ok(String::new());
        }

        with_api_scope(lua, ctx, |lua| {
            // Try as an expression first so REPL lines echo their value.
            let values = match lua
                .load(format!("return {}", trimmed))
                .eval::<mlua::MultiValue>()
            {
                Ok(v) => v,
                Err(_) => lua
                    .load(trimmed)
                    .eval::<mlua::MultiValue>()
                    .map_err(|e| e.to_string())?,
            };

            let rendered = values
                .iter()
                .map(value_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            Ok(rendered)
        })
    }
}

/// Log a script error into the borrowed console via the scoped error sink. The
/// console is owned by the scope's `RefCell`, so route the message through the
/// registered `__rusty_error` function rather than smuggling a second Rust borrow.
fn log_error(lua: &Lua, message: String) {
    if let Ok(sink) = lua.globals().get::<_, mlua::Function>(ERROR_SINK) {
        let _ = sink.call::<_, ()>(message);
    }
}

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
