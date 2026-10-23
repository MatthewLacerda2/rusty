//! An entity's material slots (#1285), Unity's `Renderer.sharedMaterials`: slot `i`
//! draws run `i` of its mesh (`MaterialComponent::slot_runs`). The one place a slot
//! is read or pointed at another library material, shared by
//! `Material.Get`/`Set`/`GetAll`/`SetAt` and the editor's Material card, so the
//! bounds check lives once.

use super::MaterialLibrary;
use crate::components::MaterialComponent;
use crate::ecs::World;

/// Entity `id`'s material keys, in slot order; empty when it has no Material.
pub fn keys(world: &World, id: u32) -> Vec<String> {
    world
        .material(id)
        .map(|m| m.materials.clone())
        .unwrap_or_default()
}

/// Point entity `id`'s material slot `slot` (0-based) at library material `key`
/// (Unity's `renderer.sharedMaterials[slot] = …`). `slot` may be one past the last,
/// which adds a slot (attaching a Material when the entity has none). A missing
/// entity, a slot further out, or a key the library lacks is an error naming it, and
/// changes nothing.
pub fn set_slot(
    world: &mut World,
    materials: &MaterialLibrary,
    id: u32,
    slot: usize,
    key: &str,
) -> Result<(), String> {
    if !world.contains(id) {
        return Err(format!("no entity {id}"));
    }
    let mut list = keys(world, id);
    if slot > list.len() {
        let count = list.len();
        return Err(format!(
            "entity {id} has {count} material slot(s); slot {slot} is out of range \
             (0..={count}, where {count} adds one)"
        ));
    }
    if !materials.contains_key(key) {
        return Err(format!("no material asset {key:?}"));
    }
    match list.get_mut(slot) {
        Some(existing) => *existing = key.to_string(),
        None => list.push(key.to_string()),
    }
    world.set_material(id, Some(MaterialComponent { materials: list }));
    Ok(())
}
