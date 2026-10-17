//! src/scene/authoring/script.rs — attaching a script behaviour (#998).
//!
//! A script is not a defaulted first-class kind: attaching one says *which* file, so
//! it has its own verb instead of a [`ComponentKind`](super::ComponentKind) row. Every
//! surface that gives an entity a script appends through [`attach_script`]: the
//! inspector's Add Component script list, the script asset's "attach to entity"
//! picker, `Scene.AddScript`, the default scene, the UI-widget builders and the dev
//! tools.

use std::collections::BTreeMap;

use crate::components::{ScriptComponent, ScriptFieldValue};
use crate::ecs::World;

/// Append the script at `path` to entity `id`'s scripts, with `values` overriding its
/// exported fields' defaults. An entity holds many scripts (#83) and may hold the same
/// one twice, as in Unity. The slot loads at play-enter, or on the next tick when
/// attached during play. `false` when there is no entity `id`.
pub fn attach_script(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::Scene;

    #[test]
    fn appends_and_refuses_a_missing_entity() {
        let mut scene = Scene::new();
        let id = scene.add_entity("Thing".to_string());
        let speed = BTreeMap::from([("speed".to_string(), ScriptFieldValue::Number(2.0))]);
        assert!(attach_script(
            &mut scene.world,
            id,
            "a.lua",
            BTreeMap::new()
        ));
        assert!(attach_script(&mut scene.world, id, "a.lua", speed.clone()));
        let scripts = scene.world.scripts(id).unwrap().clone();
        assert_eq!(
            scripts.len(),
            2,
            "a second attach appends, duplicates allowed"
        );
        assert_eq!(scripts[1].values, speed);
        assert!(!attach_script(
            &mut scene.world,
            id + 99,
            "a.lua",
            BTreeMap::new()
        ));
    }
}
