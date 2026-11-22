//! A `.prefab` file is never a user: it carries its own copy of every material its entities name.

use super::{entity_material_key, set_slot, MaterialLibrary};
use rusty_components::{Entity, MaterialComponent, ParticleEmitterComponent};
use rusty_scene::{MaterialAsset, Scene, SceneId};

/// What names a library material by key: a new kind of user joins `users`, or a delete leaves it
/// dangling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MaterialUser {
    Slot { id: u32, slot: usize },
    Particles { id: u32 },
    Decal { id: u32 },
}

#[derive(Clone, Debug)]
pub struct DeletedMaterial {
    pub scene: SceneId,
    pub key: String,
    pub asset: MaterialAsset,
    pub moved: Vec<MaterialUser>,
    pub created: Vec<String>,
}

pub fn users(scene: &Scene, key: &str) -> Vec<MaterialUser> {
    let mut ids = scene.world.ids().to_vec();
    ids.sort_unstable();
    let mut found = Vec::new();
    for id in ids {
        let material = scene.world.material(id);
        let particles = scene.world.particles(id);
        let named = entity_users(id, material.as_deref(), particles.as_deref());
        let used = named.into_iter().filter(|(_, k)| k == key);
        found.extend(used.map(|(user, _)| user));
    }
    let decals = scene.decals.iter();
    let named = decals.filter(|d| d.material.as_deref() == Some(key));
    found.extend(named.map(|d| MaterialUser::Decal { id: d.id }));
    found
}

pub fn entity_keys(entity: &Entity) -> Vec<String> {
    let named = entity_users(
        entity.id,
        entity.material.as_ref(),
        entity.particles.as_ref(),
    );
    named.into_iter().map(|(_, key)| key).collect()
}

fn entity_users(
    id: u32,
    material: Option<&MaterialComponent>,
    particles: Option<&ParticleEmitterComponent>,
) -> Vec<(MaterialUser, String)> {
    let slots = material.iter().flat_map(|m| m.materials.iter().enumerate());
    let mut found: Vec<_> = slots
        .map(|(slot, key)| (MaterialUser::Slot { id, slot }, key.clone()))
        .collect();
    if let Some(key) = particles.and_then(|p| p.render.material.clone()) {
        found.push((MaterialUser::Particles { id }, key));
    }
    found
}

pub fn drop_unused(scene: &mut Scene, keys: &[String]) {
    for key in keys {
        if users(scene, key).is_empty() {
            scene.materials.remove(key);
        }
    }
}

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

pub fn describe_all(scene: &Scene, users: &[MaterialUser]) -> String {
    let each = users.iter().map(|u| describe(scene, u));
    each.collect::<Vec<_>>().join("; ")
}

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
        log::warn!("[Material] {key:?} deleted; now on the default material: {named}");
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
        if let Err(e) = point(scene, user, Some(&key)) {
            let user = describe(scene, user);
            log::warn!("[Material] undo of {key:?}: {user} not restored: {e}");
        }
    }
    let created = deleted.created.into_iter().filter(|m| *m != key);
    drop_unused(scene, &created.collect::<Vec<_>>());
    Ok(())
}

fn move_off(scene: &mut Scene, user: &MaterialUser, created: &mut Vec<String>) {
    let MaterialUser::Slot { id, .. } = *user else {
        let _always = point(scene, user, None);
        return;
    };
    let fallback = entity_material_key(id);
    if !scene.materials.contains_key(&fallback) {
        scene
            .materials
            .insert(fallback.clone(), MaterialAsset::default());
        created.push(fallback.clone());
    }
    if let Err(e) = point(scene, user, Some(&fallback)) {
        let user = describe(scene, user);
        log::warn!("[Material] {user} not moved to {fallback:?}: {e}");
    }
}

fn point(scene: &mut Scene, user: &MaterialUser, key: Option<&str>) -> Result<(), String> {
    let key = key.map(str::to_string);
    match *user {
        MaterialUser::Slot { id, slot } => {
            let library: &MaterialLibrary = &scene.materials;
            if let Some(key) = key {
                return set_slot(&mut scene.world, library, id, slot, &key);
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
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod prefab_tests;
