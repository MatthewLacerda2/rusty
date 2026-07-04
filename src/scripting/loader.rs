//! src/scripting/loader.rs — compile + register entity scripts.
//!
//! Two entry points: [`ScriptManager::load_entity_script`] compiles one script
//! slot, and [`ScriptManager::load_new_scripts`] is the scan that drains the
//! queued-spawn contract (#322). A spawn verb (`Scene.Instantiate`,
//! `Scene.CreateEntity`, …) usually runs *mid-dispatch*, inside another
//! script's callback — loading (and `Awake`-ing) the new script synchronously
//! there would re-enter the Lua scope. Instead the spawned entity just sits in
//! the scene until the head of the next tick's script phase, when the scan
//! loads every script slot with no prior load attempt; `init_scripts` then
//! runs `Awake` + `Start` before any `Update` of that tick.

use std::collections::BTreeMap;
use std::path::Path;

use mlua::Table;

use crate::components::ScriptFieldValue;

use super::manager::{ScriptInstance, ScriptManager};

/// One script slot to load: `(entity_id, script_index, path, field values)`.
type ScriptRow = (u32, usize, String, BTreeMap<String, ScriptFieldValue>);

impl ScriptManager {
    /// Loads and runs one of an entity's scripts, then registers its lifecycle
    /// methods. `script_index` is the slot in the entity's `scripts` vec, so an
    /// entity's many scripts (#83) each get their own keyed lifecycle table.
    /// Every attempt — success or failure — is recorded in `load_attempted`, so
    /// the spawn scan never retries a slot.
    pub fn load_entity_script(
        &mut self,
        entity_id: u32,
        script_index: usize,
        script_path: &str,
        field_values: &BTreeMap<String, ScriptFieldValue>,
    ) -> Result<(), String> {
        self.load_attempted.insert((entity_id, script_index));
        let lua = self.lua.as_ref().ok_or("Lua runtime not initialized")?;

        if !Path::new(script_path).exists() {
            return Err(format!("Script file not found: {}", script_path));
        }

        let script_code = std::fs::read_to_string(script_path)
            .map_err(|e| format!("Failed to read script: {}", e))?;

        // Load the script chunk
        let chunk = lua.load(&script_code);
        let table: Table = chunk
            .eval()
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e))?;

        // Merge the inspector-set field values (#84) over the schema defaults onto
        // the lifecycle table, so `self.<field>` inside the script reads the
        // configured value. Pure data assignment — determinism-clean.
        super::schema::apply_field_values(&table, &script_code, field_values)
            .map_err(|e| format!("Failed to apply script fields for {}: {}", script_path, e))?;

        // Cache the returned lifecycle table in the Lua registry
        let reg_key = lua
            .create_registry_value(table)
            .map_err(|e| format!("Failed to register script table: {}", e))?;

        self.entity_scripts.insert(
            (entity_id, script_index),
            ScriptInstance {
                table: reg_key,
                awoken: false,
                started: false,
                enabled_last: false,
            },
        );

        Ok(())
    }

    /// Load every scene script slot with no prior load attempt. At play-enter
    /// that is every attached script; during play it is the slots runtime
    /// spawns added — the drain end of the queued-spawn contract (#322).
    /// Compile errors are logged to the console once (a failed slot is never
    /// retried). Rows load in ascending `(entity, script index)` order, so load
    /// order — and any top-level chunk side effect — is deterministic.
    pub fn load_new_scripts(&mut self) {
        if self.lua.is_none() {
            return;
        }
        let mut rows: Vec<ScriptRow> = {
            let scene = self.scene.borrow();
            scene
                .iter()
                .flat_map(|e| {
                    e.scripts
                        .iter()
                        .enumerate()
                        .filter(|&(i, _)| !self.load_attempted.contains(&(e.id, i)))
                        .map(|(i, s)| (e.id, i, s.path.clone(), s.values.clone()))
                        .collect::<Vec<_>>()
                })
                .collect()
        };
        rows.sort_unstable_by_key(|&(id, index, ..)| (id, index));
        for (id, index, path, values) in rows {
            if let Err(e) = self.load_entity_script(id, index, &path, &values) {
                let msg = format!("Lua compile error (Entity {}): {}", id, e);
                self.console.borrow_mut().error(msg);
            }
        }
    }
}
