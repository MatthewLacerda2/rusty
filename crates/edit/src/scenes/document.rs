//! One of several loaded scenes' document: its members under the file's ids and names. A
//! reference to another scene's entity is not written (Unity's rule).

use std::collections::{BTreeMap, BTreeSet};

use rusty_components::EntityRefs;
use rusty_scene::lighting::lightmap_set::LightmapSet;
use rusty_scene::lighting::occlusion::OcclusionData;
use rusty_scene::mesh_library::{scene_mesh_users, MeshLibrary};
use rusty_scene::serialize::SceneData;
use rusty_scene::{Entity, MaterialAsset, Scene};

use crate::load::additive::rename_entity_mesh;
use crate::prefab::merge::{rename_entity_material, uniquify};
use rusty_scene::runtime::membership::file::SceneFile;

pub(crate) struct Document {
    pub data: SceneData,
    pub ids: BTreeMap<u32, u32>,
    pub next_written: u32,
    pub dropped: Vec<String>,
}

pub(crate) fn document(scene: &Scene, handle: u32) -> Option<Document> {
    let file = scene.loaded.file(handle)?;
    let members = scene.members(Some(handle));
    let held: BTreeSet<u32> = members.iter().copied().collect();
    let mut entities = saved(scene, &members, file);
    let dropped = crossing(scene, &entities, &held);
    let order: Vec<u32> = entities.iter().map(|e| e.id).collect();
    let (ids, next_written) = file.file_ids(&order);
    let to = |id: u32| ids.get(&id).copied();
    let selected = scene.selected_entity_id.and_then(to);
    let lightmaps = members_lightmaps(scene, &entities);
    renumber(&mut entities, &ids);
    let active = handle == scene.loaded.active();
    let materials = file_materials(scene, file, &mut entities, active);
    let meshes = file_meshes(scene, file, &mut entities, active);
    let settings = match (active, scene.loaded.kept_settings(handle)) {
        (false, Some(kept)) => kept.clone(),
        _ => scene.settings(),
    };
    let carried = file.carried.as_deref();
    let rest = Rest::of(scene, carried, || {
        own_lightmaps(lightmaps, &ids, next_written)
    });
    let data = SceneData {
        entities,
        next_entity_id: next_written,
        selected_entity_id: selected.or(carried.and_then(|c| c.selected_entity_id)),
        skybox_path: settings.skybox_path,
        ambient_color: settings.ambient_color,
        ambient_intensity: settings.ambient_intensity,
        ambient: settings.ambient,
        nav_settings: settings.nav_settings,
        fog: settings.fog,
        sky: settings.sky,
        layers: rest.layers,
        collision_matrix: rest.collision_matrix,
        materials,
        meshes,
        probes: rest.probes,
        reflection_probes: rest.reflection_probes,
        lightmaps: rest.lightmaps,
        lighting_settings: settings.lighting_settings,
    };
    Some(Document {
        data,
        ids,
        next_written,
        dropped,
    })
}

/// The file's own first, in their order.
fn saved(scene: &Scene, members: &[u32], file: &SceneFile) -> Vec<Entity> {
    let documents = members
        .iter()
        .filter_map(|&id| scene.world.saved_document(id));
    let mut entities: Vec<Entity> = documents.collect();
    crate::prefab::live::write_overrides(&mut entities);
    rusty_scene::skeleton::strip_bones(&mut entities);
    entities.sort_by_key(|e| !file.holds(e.id));
    entities
}

struct Rest {
    layers: rusty_scene::LayerRegistry,
    collision_matrix: rusty_scene::CollisionMatrix,
    probes: rusty_scene::lighting::probe::ProbeVolume,
    reflection_probes: rusty_scene::lighting::reflection_probe::ReflectionProbeSet,
    lightmaps: LightmapSet,
}

impl Rest {
    fn of(scene: &Scene, carried: Option<&SceneData>, own: impl FnOnce() -> LightmapSet) -> Self {
        match carried {
            Some(c) => Self {
                layers: c.layers.clone(),
                collision_matrix: c.collision_matrix.clone(),
                probes: c.probes.clone(),
                reflection_probes: c.reflection_probes.clone(),
                lightmaps: c.lightmaps.clone(),
            },
            None => Self {
                layers: scene.layers.clone(),
                collision_matrix: scene.collision_matrix.clone(),
                probes: scene.probes.clone(),
                reflection_probes: scene.reflection_probes.clone(),
                lightmaps: own(),
            },
        }
    }
}

fn renumber(entities: &mut [Entity], ids: &BTreeMap<u32, u32>) {
    let to = |id: u32| ids.get(&id).copied();
    for entity in entities {
        entity.id = ids[&entity.id];
        entity.parent_id = entity.parent_id.and_then(to);
        entity.children = entity.children.iter().filter_map(|&c| to(c)).collect();
        entity.remap_refs(&to);
    }
}

/// Every set filtered to the document's members, so another scene's World ids never reach it,
/// nor collide with a member's file id in the remap.
fn members_lightmaps(scene: &Scene, entities: &[Entity]) -> LightmapSet {
    let ids: BTreeSet<u32> = entities.iter().map(|e| e.id).collect();
    let mut set = scene.lightmaps.saved().clone();
    set.keep_only(&ids);
    set
}

fn own_lightmaps(set: LightmapSet, ids: &BTreeMap<u32, u32>, next: u32) -> LightmapSet {
    let moves = rusty_scene::runtime::renumber::Moves::to_file(ids, next);
    if moves.is_empty() {
        return set;
    }
    set.remap(&moves).unwrap_or_else(|| {
        log::warn!(
            "[Scene] the first scene's lightmaps are not written: two of its entities' ids \
             collide in the file; Generate Lighting again"
        );
        LightmapSet::default()
    })
}

/// The first scene's PVS under the file's ids, its members' objects only, as its lightmaps.
pub(crate) fn own_occlusion(
    scene: &Scene,
    ids: &BTreeMap<u32, u32>,
    next: u32,
) -> Option<OcclusionData> {
    let data = scene.occlusion.as_deref()?;
    let members: BTreeSet<u32> = ids.keys().copied().collect();
    let moves = rusty_scene::runtime::renumber::Moves::to_file(ids, next);
    let own = data.keep_only(&members).and_then(|kept| kept.remap(&moves));
    if own.is_none() {
        log::warn!(
            "[Scene] the first scene's occlusion is not written: its ids collide in the file; \
             Generate Lighting again"
        );
    }
    own
}

fn crossing(scene: &Scene, entities: &[Entity], held: &BTreeSet<u32>) -> Vec<String> {
    let name = |id: u32| {
        scene
            .world
            .name(id)
            .map(|n| n.to_string())
            .unwrap_or_default()
    };
    let mut out = Vec::new();
    for entity in entities {
        entity.each_ref(&mut |field, target| {
            if held.contains(&target) || !scene.world.contains(target) {
                return;
            }
            let there = scene.member_path(target, None).unwrap_or_default();
            out.push(format!(
                "'{}' ({}) {field} names '{}' ({target}) in '{there}'",
                name(entity.id),
                entity.id,
                name(target),
            ));
        });
    }
    out
}

fn material_users(entity: &Entity) -> Vec<String> {
    let slots = entity.material.iter().flat_map(|m| &m.materials);
    let emitted = entity
        .particles
        .iter()
        .filter_map(|p| p.render.material.as_ref());
    slots.chain(emitted).cloned().collect()
}

/// The names it held, those its entities use, and (active) those no scene holds.
fn file_materials(
    scene: &Scene,
    file: &SceneFile,
    entities: &mut [Entity],
    active: bool,
) -> BTreeMap<String, MaterialAsset> {
    let used = entities.iter().flat_map(material_users);
    let library = &*scene.materials;
    let owned = owned_names(scene, |f| &f.materials);
    let (out, renames) = file_library(library, &file.materials, used, &owned, active);
    for entity in entities {
        rename_entity_material(entity, &renames);
    }
    out
}

fn file_meshes(
    scene: &Scene,
    file: &SceneFile,
    entities: &mut [Entity],
    active: bool,
) -> MeshLibrary {
    let used: Vec<String> = entities
        .iter()
        .flat_map(|e| scene_mesh_users(e.mesh.as_ref(), e.collider.as_ref()))
        .map(str::to_string)
        .collect();
    let owned = owned_names(scene, |f| &f.meshes);
    let (out, renames) = file_library(&scene.meshes, &file.meshes, used, &owned, active);
    for entity in entities {
        rename_entity_mesh(entity, &renames);
    }
    out
}

fn owned_names(
    scene: &Scene,
    names: impl Fn(&SceneFile) -> &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let files = scene.loaded.handles().into_iter();
    let files = files.filter_map(|h| scene.loaded.file(h));
    files.flat_map(|f| names(f).keys().cloned()).collect()
}

fn file_library<V: Clone>(
    library: &BTreeMap<String, V>,
    held: &BTreeMap<String, String>,
    used: impl IntoIterator<Item = String>,
    owned: &BTreeSet<String>,
    active: bool,
) -> (BTreeMap<String, V>, BTreeMap<String, String>) {
    let mut out = BTreeMap::new();
    let mut renames = held.clone();
    for (world, file) in held {
        if let Some(value) = library.get(world) {
            out.insert(file.clone(), value.clone());
        }
    }
    let orphans = library
        .keys()
        .filter(|k| active && !owned.contains(*k))
        .cloned();
    for world in used.into_iter().chain(orphans) {
        if renames.contains_key(&world) {
            continue;
        }
        let Some(value) = library.get(&world) else {
            continue;
        };
        let name = match out.contains_key(&world) {
            false => world.clone(),
            true => uniquify(&out, &world),
        };
        out.insert(name.clone(), value.clone());
        renames.insert(world, name);
    }
    renames.retain(|world, file| world != file);
    (out, renames)
}
