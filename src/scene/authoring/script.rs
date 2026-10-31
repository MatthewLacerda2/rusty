//! src/scene/authoring/script.rs — attaching a script component (#996).
//!
//! The one way a script lands on an entity, shared by the inspector's Add
//! Component menu, the script asset inspector's entity picker, the widget builders, the
//! default scene, the dev tools (stress bench, bot-player) and `Scene.AddScript`,
//! so they can never disagree about what an attached script looks like.

use std::collections::BTreeMap;

use crate::components::{ScriptComponent, ScriptFieldValue};
use crate::ecs::World;

/// Append the script at `path` (project-relative, like the inspector stores it) to
/// entity `id`'s scripts, with `values` set over its schema defaults. An entity
/// holds many scripts (#83), so this never replaces one. In Play the new slot
/// loads at the head of the next tick, like a spawned entity's. Returns whether
/// the entity exists.
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
