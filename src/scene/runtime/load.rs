//! src/scene/runtime/load.rs — the scene half of runtime scene loading (#432).
//!
//! Play-mode `Scene.Load(path)` only records the request ([`Scene::request_load`]);
//! the tick's scene-load phase (after the destroy phase) takes it, runs the outgoing
//! scene's teardown callbacks, then calls [`Scene::swap_in`] — so the World is never
//! replaced mid-dispatch and a replay swaps on the same tick every time.
//!
//! One active scene stays the model. `Scene.DontDestroyOnLoad(id)` marks an entity
//! ([`Scene::dont_destroy_on_load`]); at the swap it and every descendant are carried
//! into the incoming World **with their ids**, because scripts, timers and the UI
//! event system hold those ids. An incoming entity whose id a survivor already holds
//! is moved to a fresh id instead (`make_room`), its references rewritten with it
//! through the one id-field walk; a survivor's references to the entities left
//! behind are cleared through the same walk, and the level's baked PVS renames its
//! objects with the moves (#1714).

use std::collections::{BTreeMap, BTreeSet};

use super::prepare::PreparedScene;
use crate::components::EntityRefs;
use crate::scene::serialize::{apply_rehydrated, SceneData};
use crate::scene::{Entity, MaterialAsset, Scene};

impl Scene {
    /// Queue a play-mode load of `path`, swapped in at the tick's tail. A later
    /// request replaces an earlier one: the last `Scene.Load` wins (`runtime::loads`).
    pub fn request_load(&mut self, path: String) {
        self.loads.request(path);
    }

    /// Mark `id` to survive scene loads, with its descendants. `false` when no entity
    /// has that id. The mark lasts until the entity is destroyed or play stops.
    pub fn dont_destroy_on_load(&mut self, id: u32) -> bool {
        let exists = self.world.contains(id);
        if exists {
            self.persistent.insert(id);
        }
        exists
    }

    /// The entities a load carries across: every live marked entity and all of its
    /// descendants.
    pub fn load_survivors(&self) -> BTreeSet<u32> {
        self.persistent
            .iter()
            .flat_map(|&id| self.world.self_and_descendants(id))
            .collect()
    }

    /// Replace the World with `data` (the document read from `path`), carrying
    /// `survivors` across unchanged: same ids, same components, same order among
    /// themselves, appended after the incoming entities. A survivor whose parent does
    /// not survive becomes a root. The materials survivors reference come along
    /// unless the incoming library already defines that name, and so do the library
    /// meshes they draw (#2059). Persistent decals
    /// and the survivors' ragdoll state (#1895) come along (#1518,
    /// `scene::decal::persist`); the rest, and queued destroys
    /// of the outgoing scene, are dropped, and the scene takes a fresh
    /// runtime identity — it is a different scene now.
    pub fn swap_in(
        &mut self,
        data: SceneData,
        path: &str,
        survivors: &BTreeSet<u32>,
    ) -> Result<(), String> {
        self.swap_in_prepared(PreparedScene::from_data(data, path), survivors)
    }

    /// [`Scene::swap_in`] for a scene already prepared from its files: the
    /// activation half of a load (`scene::runtime::prepare`, #2123). Only what
    /// depends on the survivors and the live scene happens here.
    pub fn swap_in_prepared(
        &mut self,
        prepared: PreparedScene,
        survivors: &BTreeSet<u32>,
    ) -> Result<(), String> {
        let PreparedScene {
            path: _,
            mut document,
            sidecars,
            baked_navmesh: _, // activation's, taken before (`app::scene_load`)
        } = prepared;
        let carried: Vec<Entity> = self
            .entity_ids()
            .into_iter()
            .filter(|id| survivors.contains(id))
            .filter_map(|id| self.world.entity_document(id))
            .collect();
        let materials: BTreeMap<String, MaterialAsset> = carried
            .iter()
            .filter_map(|e| e.material.as_ref())
            .flat_map(|m| &m.materials)
            .filter_map(|key| Some((key.clone(), self.materials.get(key)?.clone())))
            .collect();

        let meshes = carried_meshes(self, &carried);
        let decals = self.take_persistent_decals(survivors);
        let mut ragdolls = std::mem::take(&mut self.ragdolls);
        ragdolls.retain(|id| survivors.contains(&id));
        let moved = make_room(document.data_mut(), survivors);
        apply_rehydrated(self, document);
        self.ragdolls = ragdolls;
        for mut entity in carried {
            if entity.parent_id.is_some_and(|p| !survivors.contains(&p)) {
                entity.parent_id = None;
            }
            // The outgoing scene is destroyed: what a survivor named in it is gone,
            // as after a despawn (#1264), and its id may now be an incoming entity's.
            entity.forget(&|r| !survivors.contains(&r));
            self.world.insert_entity(entity);
        }
        for (name, material) in materials {
            self.materials.entry(name).or_insert(material);
        }
        for (name, mesh) in meshes {
            self.meshes.entry(name).or_insert(mesh);
        }
        self.sync_scene_meshes();
        self.persistent.retain(|id| survivors.contains(id));
        self.history_cuts.retain(|id| survivors.contains(&id));
        self.pending_destroy.retain(|id| survivors.contains(id));
        self.land_persistent_decals(decals);
        self.renew_id();
        crate::scene::nav_settings::apply_navmesh(self, sidecars.navmesh);
        let loaded = crate::scene::lighting::io::apply_lighting_sidecar(self, sidecars.lighting);
        self.remap_occlusion(&moved);
        loaded
    }

    /// The baked PVS was baked against the file's ids: rename the objects
    /// `make_room` moved, so a carried entity never turns the level's culling off.
    fn remap_occlusion(&mut self, moved: &BTreeMap<u32, u32>) {
        if moved.is_empty() {
            return;
        }
        let Some(data) = self.occlusion.as_deref() else {
            return;
        };
        let remapped = data.remap(moved);
        if remapped.is_none() {
            log::warn!(
                "[Occlusion] the bake's ids collide after the load: nothing is occlusion-culled"
            );
        }
        *self.occlusion = remapped.map(std::sync::Arc::new);
    }
}

/// The library meshes (#2059) `carried` use (a Mesh, or a Mesh collider's kept
/// copy), to come along like their materials.
fn carried_meshes(scene: &Scene, carried: &[Entity]) -> crate::scene::mesh_library::MeshLibrary {
    use crate::scene::mesh_library::scene_mesh_users;
    let names = carried
        .iter()
        .flat_map(|e| scene_mesh_users(e.mesh.as_ref(), e.collider.as_ref()));
    let found = names.filter_map(|n| Some((n.to_string(), scene.meshes.get(n)?.clone())));
    found.collect()
}

/// Move every incoming entity whose id is in `taken` to a fresh id past both the
/// document's ids and `taken`, in document order (so the result is a pure function of
/// the inputs), rewriting parent, child and component references to match. Returns
/// the moves, old id → new.
pub(super) fn make_room(data: &mut SceneData, taken: &BTreeSet<u32>) -> BTreeMap<u32, u32> {
    let mut next = data
        .entities
        .iter()
        .map(|e| e.id + 1)
        .chain(taken.iter().map(|&id| id + 1))
        .fold(data.next_entity_id, u32::max);
    let mut moved = BTreeMap::new();
    for entity in &data.entities {
        if taken.contains(&entity.id) {
            moved.insert(entity.id, next);
            next += 1;
        }
    }
    data.next_entity_id = next;
    if moved.is_empty() {
        return moved;
    }
    let remap = |id: u32| moved.get(&id).copied().unwrap_or(id);
    for entity in &mut data.entities {
        entity.id = remap(entity.id);
        entity.parent_id = entity.parent_id.map(remap);
        for child in &mut entity.children {
            *child = remap(*child);
        }
        entity.remap_refs(&|r| Some(remap(r)));
    }
    data.selected_entity_id = data.selected_entity_id.map(remap);
    moved
}
