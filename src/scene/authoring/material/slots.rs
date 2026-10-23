//! An entity's material slots (#1285): one per sub-mesh run of its mesh, Unity's
//! `Renderer.sharedMaterials`. The one place a slot is read or pointed at another
//! library material, shared by `Material.Get`/`Set`/`GetAll`/`SetAt` and the
//! editor's Material card, so the bounds check lives once.

use super::MaterialLibrary;
use crate::components::MaterialComponent;
use crate::ecs::World;

/// How many material slots entity `id` has: one per index run of its mesh, or the
/// length of its material list when that is longer, and at least one.
pub fn slot_count(world: &World, id: u32) -> usize {
    let runs = world.mesh(id).map_or(1, |m| m.index_ranges().count());
    let listed = world.material(id).map_or(0, |m| m.materials.len());
    runs.max(listed).max(1)
}

/// Entity `id`'s material keys, one per slot ([`slot_count`]): a slot its list does
/// not reach draws with the first material, so it reads as that. Empty when it has
/// no Material.
pub fn keys(world: &World, id: u32) -> Vec<String> {
    let Some(material) = world.material(id) else {
        return Vec::new();
    };
    let count = slot_count(world, id);
    (0..count)
        .map(|slot| material.for_range(slot).to_string())
        .collect()
}

/// Point entity `id`'s material slot `slot` at library material `key` (Unity's
/// `renderer.sharedMaterials[slot] = …`), attaching a Material when it has none. A
/// missing entity, a slot past [`slot_count`] or a key the library lacks is an
/// error naming it, and changes nothing.
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
    list.resize(count, key.to_string());
    list[slot] = key.to_string();
    world.set_material(id, Some(MaterialComponent { materials: list }));
    Ok(())
}
