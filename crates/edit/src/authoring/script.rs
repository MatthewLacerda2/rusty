use std::collections::BTreeMap;

use rusty_components::{ScriptComponent, ScriptFieldValue};
use rusty_ecs::World;

pub fn add_script(
    world: &mut World,
    id: u32,
    path: &str,
    values: BTreeMap<String, ScriptFieldValue>,
) -> bool {
    let Some(mut scripts) = world.scripts_mut(id) else {
        return false;
    };
    scripts.push(ScriptComponent {
        path: path.to_string(),
        is_loaded: false,
        values,
    });
    true
}
