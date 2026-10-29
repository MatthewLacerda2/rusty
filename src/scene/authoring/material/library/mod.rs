//! Deleting a material from the scene's library (#1576), Unity's deleting a material
//! asset. The one place the engine knows what names a library material by its key
//! (an entity's material slots, its mesh particles, a live decal), shared by
//! `Material.DeleteAsset` and the editor's Material card.
//!
//! A material still in use is refused, naming its users; with `force` each user
//! falls back to the engine's default material: a slot to its entity's own
//! default (`entity_{id}_material`, created as [`MaterialAsset::default`] when the
//! library has none, the material *Add Component* attaches), mesh particles and a
//! decal to no material, which draw the engine default and the decal's texture.
//! The [`DeletedMaterial`] a delete returns is one undo step ([`restore_asset`]).
//!
//! A `.prefab` file is never a user: it carries its own copy of every material it
//! names (`PrefabData::materials`), and instancing it merges that copy back in.
//! An instance in the scene is a user through its entities' slots.
//!
//! Pure: no wall-clock, no RNG; users are listed in entity-id order.

use super::{entity_material_key, set_slot, MaterialLibrary};
use crate::scene::{MaterialAsset, Scene, SceneId};

/// One place that names a library material by its key.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterialUser {
    /// Material slot `slot` of entity `id`.
    Slot { id: u32, slot: usize },
    /// Entity `id`'s mesh particles (`render.material`).
    Particles { id: u32 },
    /// Live decal `id` (`Decals.Spawn`'s handle).
    Decal { id: u32 },
}

/// What a delete took out and moved, so [`restore_asset`] can put it back.
#[derive(Clone, Debug)]
pub struct DeletedMaterial {
    /// The scene it was deleted from; a restore into another one is refused.
    pub scene: SceneId,
    pub key: String,
    pub asset: MaterialAsset,
    /// The users moved off it, as they were.
    pub moved: Vec<MaterialUser>,
    /// The entity default materials the delete had to create for a moved slot.
    pub created: Vec<String>,
}

/// Every user of library material `key`, entities in id order (slots, then
/// particles), then live decals oldest first.
pub fn users(scene: &Scene, key: &str) -> Vec<MaterialUser> {
    let mut ids = scene.world.ids().to_vec();
    ids.sort_unstable();
    let mut found = Vec::new();
    for id in ids {
        if let Some(material) = scene.world.material(id) {
            let slots = material.materials.iter().enumerate();
            let used = slots.filter(|(_, k)| *k == key);
            found.extend(used.map(|(slot, _)| MaterialUser::Slot { id, slot }));
        }
        let particles = scene.world.particles(id);
        if particles.is_some_and(|p| p.render.material.as_deref() == Some(key)) {
            found.push(MaterialUser::Particles { id });
        }
    }
    let decals = scene.decals.iter();
    let named = decals.filter(|d| d.material.as_deref() == Some(key));
    found.extend(named.map(|d| MaterialUser::Decal { id: d.id }));
    found
}

/// A user as a person reads it: `entity 3 "Crate" slot 0`, with the prefab an
/// instance's entity comes from.
pub fn describe(scene: &Scene, user: &MaterialUser) -> String {
    let entity = |id: u32| {
        let name = scene.world.name(id).map(|n| n.clone()).unwrap_or_default();
        let link = scene.world.prefab_link(id).map(|l| l.source.clone());
        let prefab = link
            .map(|p| format!(" (instance of {p})"))
            .unwrap_or_default();
        format!("entity {id} {name:?}{prefab}")
    };
    match *user {
        MaterialUser::Slot { id, slot } => format!("{} slot {slot}", entity(id)),
        MaterialUser::Particles { id } => format!("{} particles", entity(id)),
        MaterialUser::Decal { id } => format!("decal {id}"),
    }
}

/// [`describe`] each of `users`, joined by `"; "`.
pub fn describe_all(scene: &Scene, users: &[MaterialUser]) -> String {
    let each = users.iter().map(|u| describe(scene, u));
    each.collect::<Vec<_>>().join("; ")
}

/// Delete library material `key`. One still in use is refused with its users
/// named, unless `force`, which moves each to the engine's default material (the
/// module docs) and warns in the console which. An absent key is an error.
pub fn delete_asset(scene: &mut Scene, key: &str, force: bool) -> Result<DeletedMaterial, String> {
    if !scene.materials.contains_key(key) {
        return Err(format!("no material asset {key:?}"));
    }
    let moved = users(scene, key);
    let named = describe_all(scene, &moved);
    if !moved.is_empty() && !force {
        return Err(format!(
            "material asset {key:?} is used by {named}; \
             pass {{ force = true }} to move them to the default material"
        ));
    }
    let asset = scene.materials.remove(key).unwrap_or_default();
    let mut created = Vec::new();
    for user in &moved {
        move_off(scene, user, &mut created);
    }
    if !moved.is_empty() {
        log::warn!("Material {key:?} deleted; now on the default material: {named}");
    }
    let scene_id = scene.id();
    Ok(DeletedMaterial {
        scene: scene_id,
        key: key.to_string(),
        asset,
        moved,
        created,
    })
}

/// Undo a delete: put the asset back under its key, point every user it moved at
/// it again (one that is gone since is skipped), and drop the default materials it
/// created that nothing uses now. Refused for a scene other than the one it came
/// from.
pub fn restore_asset(scene: &mut Scene, deleted: DeletedMaterial) -> Result<(), String> {
    if deleted.scene != scene.id() {
        return Err(format!(
            "material asset {:?} was deleted from another scene",
            deleted.key
        ));
    }
    let key = deleted.key;
    scene.materials.insert(key.clone(), deleted.asset);
    for user in &deleted.moved {
        point(scene, user, Some(&key));
    }
    for made in deleted.created.iter().filter(|m| **m != key) {
        if users(scene, made).is_empty() {
            scene.materials.remove(made);
        }
    }
    Ok(())
}

/// Move `user` to its default material, noting any entity default it creates.
fn move_off(scene: &mut Scene, user: &MaterialUser, created: &mut Vec<String>) {
    let MaterialUser::Slot { id, .. } = *user else {
        return point(scene, user, None);
    };
    let fallback = entity_material_key(id);
    if !scene.materials.contains_key(&fallback) {
        scene
            .materials
            .insert(fallback.clone(), MaterialAsset::default());
        created.push(fallback.clone());
    }
    point(scene, user, Some(&fallback));
}

/// Point `user` at `key`; `None` (particles and decals only) leaves it on none.
fn point(scene: &mut Scene, user: &MaterialUser, key: Option<&str>) {
    let key = key.map(str::to_string);
    match *user {
        MaterialUser::Slot { id, slot } => {
            let library: &MaterialLibrary = &scene.materials;
            if let Some(key) = key {
                let _gone = set_slot(&mut scene.world, library, id, slot, &key);
            }
        }
        MaterialUser::Particles { id } => {
            if let Some(mut particles) = scene.world.particles_mut(id) {
                particles.render.material = key;
            }
        }
        MaterialUser::Decal { id } => {
            scene.decals.set_material(id, key);
        }
    }
}

#[cfg(test)]
mod tests;
