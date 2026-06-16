use mlua::Table;

use super::manager::ScriptManager;

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

        for key in keys {
            let id = key.0;
            let Some(reg) = self.entity_scripts.get(&key) else {
                continue;
            };
            let Ok(table) = lua.registry_value::<Table>(reg) else {
                continue;
            };
            let Ok(start_fn) = table.get::<_, mlua::Function>("Start") else {
                continue;
            };
            if let Err(e) = start_fn.call::<_, ()>(id) {
                self.console
                    .borrow_mut()
                    .error(format!("[Lua Error] Start on entity {} failed: {}", id, e));
            }
        }
    }

    /// Invokes the Update function on all loaded entity scripts
    pub fn update_scripts(&mut self, delta_time: f32) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        // Keep scripts whose owning entity is present and active.
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
        drop(scene);
        // Sorted by (entity, script index) so per-frame Update order is
        // deterministic (HashMap iteration order varies per run). Gameplay now
        // happens inside scripts — e.g. the weapon's Physics.Shoot vs. the enemy's
        // animation Update race in the kill frame — so a stable order is what keeps
        // replays byte-identical.
        keys.sort_unstable();

        for key in keys {
            let id = key.0;
            let Some(reg) = self.entity_scripts.get(&key) else {
                continue;
            };
            let Ok(table) = lua.registry_value::<Table>(reg) else {
                continue;
            };
            let Ok(update_fn) = table.get::<_, mlua::Function>("Update") else {
                continue;
            };
            if let Err(e) = update_fn.call::<_, ()>((id, delta_time)) {
                self.console
                    .borrow_mut()
                    .error(format!("[Lua Error] Update on entity {} failed: {}", id, e));
            }
        }
    }

    /// Invokes the OnTrigger callback on scripts of entities involved in a trigger overlap
    pub fn dispatch_trigger_events(&mut self, events: Vec<(u32, u32)>) {
        let lua = match &self.lua {
            Some(l) => l,
            None => return,
        };

        for (id_a, id_b) in events {
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
                    let Ok(trigger_fn) = table.get::<_, mlua::Function>("OnTrigger") else {
                        continue;
                    };
                    if let Err(e) = trigger_fn.call::<_, ()>((id, other)) {
                        self.console.borrow_mut().error(format!(
                            "[Lua Error] OnTrigger on entity {} failed: {}",
                            id, e
                        ));
                    }
                }
            }
        }
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
