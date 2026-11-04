//! src/scripting/loader/ — compile + register entity scripts.
//!
//! Two entry points: [`ScriptManager::load_entity_script`] makes one script
//! slot's instance from its class (`classes`: read and compiled once per VM,
//! #1710), and [`ScriptManager::load_new_scripts`] drains the queued-spawn
//! contract (#322). A spawn verb (`Scene.Instantiate`,
//! `Scene.CreateEntity`, …) usually runs *mid-dispatch*, inside another
//! script's callback — loading (and `Awake`-ing) the new script synchronously
//! there would run its chunk and hooks in the middle of the dispatch loop over
//! the manager's instances. Instead the spawned entity just sits in the scene
//! until the head of the next tick's script phase, when the drain loads every
//! script slot with no prior load attempt among the entities the World's
//! `scripts` change log stamped; `init_scripts` then runs `Awake` +
//! `Start` before any `Update` of that tick.

mod classes;

use mlua::Table;
use std::collections::BTreeMap;

use crate::components::ScriptFieldValue;
use crate::ecs::{Changes, World};

pub(super) use classes::ScriptClasses;

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
        let file = self.resolve_script(script_path);
        let console = &self.cells.console;
        let class = self.classes.get(lua, script_path, &file, |msg| {
            console.borrow_mut().warn(msg);
        })?;

        // A fresh table per instance: the chunk's top level runs again.
        let table: Table = class
            .chunk
            .call(())
            .map_err(|e| format!("Syntax error compiling {}: {}", script_path, e))?;

        // Merge the inspector-set field values (#84) over the schema defaults onto
        // the lifecycle table, so `self.<field>` inside the script reads the
        // configured value. Pure data assignment — determinism-clean.
        super::schema::apply_field_values(&table, &class.fields, field_values)
            .map_err(|e| format!("Failed to apply script fields for {}: {}", script_path, e))?;

        // Index it by entity, so `Scene.GetScript` can hand it to other scripts.
        super::instances::record(
            lua,
            entity_id,
            script_index,
            &super::script_label(script_path),
            &table,
        )
        .map_err(|e| format!("Failed to index script {}: {}", script_path, e))?;

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
    /// Compile errors are logged to the console once per slot (a failed slot is
    /// never retried). Rows load in ascending `(entity, script index)` order, so
    /// load order — and any top-level chunk side effect — is deterministic.
    ///
    /// The queue is the World's `scripts` change log (#1705, #1710): only entities
    /// whose script list was written, or that were placed, since the last drain
    /// are looked at, plus those whose load records a destroy or an unload
    /// dropped. The first drain of a VM, and any the log cannot answer (a scene
    /// load, a Play snapshot restored, an Undo), scan every entity.
    pub fn load_new_scripts(&mut self) {
        if self.lua.is_none() {
            return;
        }
        let mut rows: Vec<ScriptRow> = {
            let cell = std::rc::Rc::clone(&self.cells.scene);
            let mut scene = cell.borrow_mut();
            let ids = self.queued_ids(&mut scene.world);
            ids.into_iter()
                .flat_map(|id| {
                    scene
                        .world
                        .scripts(id)
                        .map(|scripts| {
                            scripts
                                .iter()
                                .enumerate()
                                .filter(|&(i, _)| !self.load_attempted.contains(&(id, i)))
                                .map(|(i, s)| (id, i, s.path.clone(), s.values.clone()))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                })
                .collect()
        };
        rows.sort_unstable_by_key(|&(id, index, ..)| (id, index));
        for (id, index, path, values) in rows {
            if let Err(e) = self.load_entity_script(id, index, &path, &values) {
                let msg = format!("Lua compile error (Entity {}): {}", id, e);
                self.cells.console.borrow_mut().error(msg);
            }
        }
    }

    /// The entities that may hold a script slot not yet loaded: what the `scripts`
    /// log stamped since the last drain and what was requeued, or every entity when
    /// the log cannot say.
    fn queued_ids(&mut self, world: &mut World) -> Vec<u32> {
        let requeued = std::mem::take(&mut self.requeued);
        let changes = match &mut self.spawn_cursor {
            Some(cursor) => world.changes(cursor),
            None => {
                self.spawn_cursor = Some(world.change_cursor());
                Changes::Rebuild
            }
        };
        match changes {
            Changes::Rebuild => world.ids().to_vec(),
            Changes::Ids(mut ids) => {
                ids.extend(requeued);
                ids.sort_unstable();
                ids.dedup();
                ids
            }
        }
    }
}
