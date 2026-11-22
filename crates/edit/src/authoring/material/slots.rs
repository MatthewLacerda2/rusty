use super::MaterialLibrary;
use rusty_components::MaterialComponent;
use rusty_ecs::World;

pub fn slot_count(world: &World, id: u32) -> usize {
    let runs = world.mesh(id).map_or(1, |m| m.index_ranges().count());
    let listed = world.material(id).map_or(0, |m| m.materials.len());
    runs.max(listed).max(1)
}

/// Unity's `sharedMaterials`: the list as stored, which may be shorter than [`slot_count`] (a run
/// past its end is not drawn).
pub fn keys(world: &World, id: u32) -> Vec<String> {
    world
        .material(id)
        .map(|m| m.materials.clone())
        .unwrap_or_default()
}

/// Unity's `renderer.sharedMaterials[slot] = …`. A slot past the end grows the list, each new slot
/// taking `key`.
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
    let count = slot_count(world, id);
    if slot >= count {
        return Err(format!(
            "entity {id} has {count} material slot(s); slot {slot} is out of range"
        ));
    }
    if !materials.contains_key(key) {
        return Err(format!("no material asset {key:?}"));
    }
    let mut list = keys(world, id);
    if list.len() <= slot {
        list.resize(slot + 1, key.to_string());
    }
    list[slot] = key.to_string();
    world.set_material(id, Some(MaterialComponent { materials: list }));
    Ok(())
}
