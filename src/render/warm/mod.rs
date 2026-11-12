//! src/render/warm/ — a scene load's GPU warm-up (#2123): the resources a prepared
//! scene draws with, made while the scene before it still draws, so the new scene's
//! first frame finds them and uploads nothing.
//!
//! [`Warming::plan`] lists what the document references, through the renderer's
//! caches keyed by asset, never by entity: each mesh identity's buffers (at the
//! newest revision the document carries, which activation moves into the world
//! unchanged, so the extract finds them resident), every material map as the slot
//! reading it holds it, the sky's panorama, and the lightmap page arrays. A step
//! ([`Renderer::warm_step`]) makes one, whole: a resize, a lost surface or a Stop
//! between two steps leaves nothing half-built. A warmed resource the activated scene
//! does not use (a load replaced) stays in its cache as any unused entry does.
//!
//! What it does not warm, and the new scene's first frame still makes: the material
//! bind groups and object records (kept per scene, and the scene's identity is new
//! at the swap), the reflection probes' cubes (copied into the one cube array the
//! frame places them in), and the textures of particles, ribbons, decals and UI.
//!
//! Nothing here reads a clock: the host slices the steps by its frame budget
//! (`shell::frame::load::warm`), and the sim only learns when the warm-up is done, as an
//! input (`app::SceneLoader::finished`).

use crate::asset::MapRef;
use crate::components::MeshRevision;
use crate::render::draw::materials::material_maps;
use crate::render::gpu::textures::TextureKind;
use crate::render::stats::Uploads;
use crate::render::{MeshId, Renderer};
use crate::scene::serialize::SceneData;

/// One resource to make.
#[derive(Debug, PartialEq)]
enum Item {
    /// The geometry of entity `entities[index]`, the newest of its identity.
    Mesh(usize),
    Map(MapRef, TextureKind),
    Sky(String),
    /// The colour, direction and shadowmask page arrays.
    Lightmaps,
}

/// What is left to warm of one prepared scene.
#[derive(Debug)]
pub struct Warming {
    items: Vec<Item>,
    next: usize,
    /// What the steps so far uploaded.
    pub uploads: Uploads,
}

impl Warming {
    /// Everything `data` (a prepared scene's document) draws with, once each.
    pub fn plan(data: &SceneData) -> Self {
        let mut items = meshes(data);
        for entity in &data.entities {
            let names = entity.material.iter().flat_map(|m| m.materials.iter());
            let maps = names.filter_map(|name| data.materials.get(name));
            for (map, kind) in maps.flat_map(|m| material_maps(Some(m))) {
                push_new(&mut items, Item::Map(map, kind));
            }
        }
        if !data.skybox_path.is_empty() {
            items.push(Item::Sky(data.skybox_path.clone()));
        }
        // Directions and shadowmasks are bound only beside colour pages.
        if !data.lightmaps.pages.is_empty() {
            items.push(Item::Lightmaps);
        }
        Self {
            items,
            next: 0,
            uploads: Uploads::default(),
        }
    }

    /// How much is warmed, from 0 to 1 (1 for a scene with nothing to warm).
    pub fn progress(&self) -> f32 {
        match self.items.len() {
            0 => 1.0,
            n => self.next as f32 / n as f32,
        }
    }

    /// Whether every resource is made.
    pub fn is_done(&self) -> bool {
        self.next >= self.items.len()
    }

    /// How many resources the plan holds.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Whether the plan holds nothing.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// One mesh item per geometry identity in `data`, naming the entity carrying its
/// newest revision, in the order the identities first appear.
fn meshes(data: &SceneData) -> Vec<Item> {
    let mut newest: Vec<(MeshId, MeshRevision, usize)> = Vec::new();
    let with_mesh = data.entities.iter().enumerate();
    let with_mesh = with_mesh.filter_map(|(i, e)| e.mesh.as_ref().map(|m| (i, m)));
    for (i, mesh) in with_mesh {
        if !crate::render::debug_meshes::has_geometry(&mesh.vertices, &mesh.indices) {
            continue;
        }
        let id = MeshId::from_mesh(mesh);
        match newest.iter_mut().find(|(known, ..)| *known == id) {
            Some(entry) if mesh.revision > entry.1 => (entry.1, entry.2) = (mesh.revision, i),
            Some(_) => {}
            None => newest.push((id, mesh.revision, i)),
        }
    }
    newest.into_iter().map(|(.., i)| Item::Mesh(i)).collect()
}

/// Push `item` unless it is listed already.
fn push_new(items: &mut Vec<Item>, item: Item) {
    if !items.contains(&item) {
        items.push(item);
    }
}

impl Renderer {
    /// Make the next resource `warming` lists of `data`, the document it was planned
    /// from. Returns whether one was made; `false` once the plan is done, or while
    /// its next resource is still read on a worker (the lightmap pages), when the
    /// host tries again on a later frame. What a step uploads is the warm-up's,
    /// never counted in a frame's `asset_uploads`.
    pub fn warm_step(&mut self, data: &SceneData, warming: &mut Warming) -> bool {
        let Some(item) = warming.items.get(warming.next) else {
            return false;
        };
        let frame = std::mem::take(&mut self.uploads);
        let pages = std::mem::take(&mut self.lightmaps.uploads);
        let made = self.make(data, item);
        warming
            .uploads
            .add_all(std::mem::replace(&mut self.uploads, frame));
        warming
            .uploads
            .add_all(std::mem::replace(&mut self.lightmaps.uploads, pages));
        warming.next += usize::from(made);
        made
    }

    /// Make `item`; whether it is made.
    fn make(&mut self, data: &SceneData, item: &Item) -> bool {
        match item {
            Item::Mesh(index) => {
                let mesh = data.entities[*index].mesh.as_ref().expect("planned");
                let id = MeshId::from_mesh(mesh);
                let resident = self.gpu_meshes.get(&id).map(|g| g.revision);
                if resident.is_none_or(|r| r < mesh.revision) {
                    self.update_gpu_mesh(id.clone(), mesh.revision, &mesh.vertices, &mesh.indices);
                    self.attach_clusters(&id, mesh.clusters());
                }
                true
            }
            Item::Map(map, kind) => {
                self.load_texture_as(map, *kind);
                true
            }
            Item::Sky(path) => {
                self.load_texture(&MapRef::from(path.as_str()));
                true
            }
            Item::Lightmaps => {
                let set = &data.lightmaps;
                let lists = [&set.pages[..], &set.directions[..], &set.masks[..]];
                self.lightmaps.warm(&self.device, &self.queue, lists)
            }
        }
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod plan_tests;
#[cfg(test)]
#[path = "warm_gpu_tests.rs"]
mod warm_gpu_tests;
