mod classes;

use mlua::Table;
use std::collections::BTreeMap;

use rusty_components::ScriptFieldValue;
use rusty_ecs::{Changes, World};

pub(super) use classes::ScriptClasses;

use super::manager::{ScriptInstance, ScriptManager};

type ScriptRow = (u32, usize, String, BTreeMap<String, ScriptFieldValue>);

impl ScriptManager {
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

        let table: Table = class
            .run(lua)
            .map_err(|e| format!("Error running {}: {}", script_path, e))?;

        super::schema::apply_field_values(&table, &class.fields, field_values)
            .map_err(|e| format!("Failed to apply script fields for {}: {}", script_path, e))?;

        super::instances::record(
            lua,
            entity_id,
            script_index,
            &super::script_label(script_path),
            &table,
        )
        .map_err(|e| format!("Failed to index script {}: {}", script_path, e))?;

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
