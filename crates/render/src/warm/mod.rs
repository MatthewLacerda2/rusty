//! crates/render/src/warm/ — a scene load's GPU warm-up (#2123): the resources a prepared
//! scene draws with, made while the scene before it still draws, so the new scene's
//! first frame finds them and uploads nothing.
//!
//! [`Warming::plan`] lists what the document references, through the renderer's
//! caches keyed by asset, never by entity: each mesh identity's buffers (the slots
//! the extract's own rule, `debug_meshes::stale_uploads`, would upload, at the
//! revision activation moves into the world), every material map as the slot
//! reading it holds it, the sky's panorama, and the lightmap page arrays. It runs
//! on the load's worker (`shell::frame::load::worker`), never on the tick thread.
//! A primitive's revision is one per geometry for the process
//! (`scene::authoring::primitive_revision`) and an import's is its import's, so the
//! linked prefab instances activation rebuilds from their `.prefab` draw the
//! geometry planned from the saved document, and a reload's resident meshes are
//! skipped, never rewritten under the scene still drawing. (A `.prefab` edited
//! since the scene was saved can name a mesh the plan missed: the first frame
//! uploads that one.)
//!
//! A step ([`Renderer::warm_step`]) makes one resource, whole: a resize, a lost
//! surface or a Stop between two steps leaves nothing half-built. A texture is read,
//! decoded and fitted on a thread of its own (`decode`), and its step only uploads
//! it; a step that meets one still being read, or lightmap pages still being read,
//! makes nothing, and the host tries again on a later frame. What one step costs the
//! frame is then one GPU upload: a mesh's buffers, a texture's levels and its mips
//! drawn, or a page array. A warmed resource the activated scene does not use (a
//! load replaced) stays in its cache as any unused entry does.
//!
//! What it does not warm, and the new scene's first frame still makes: the material
//! bind groups and object records (kept per scene, and the scene's identity is new
//! at the swap), the reflection probes' cubes (copied into the one cube array the
//! frame places them in), the meshes of the scene's own library (`Mesh.Create`,
//! filled at activation by `sync_scene_meshes`), the sky's panorama bind group and
//! its reflection cube's prefilter when the sky changes, and the textures of
//! particles, ribbons, decals and UI.
//!
//! Nothing here reads a clock: the host slices the steps by its frame budget
//! (`shell::frame::load::warm`), and the sim only learns when the warm-up is done, as an
//! input (`app::SceneLoader::finished`).

mod decode;

use std::collections::BTreeSet;

use crate::debug_meshes::{has_geometry, stale_uploads, UploadCandidate};
use crate::draw::materials::material_maps;
use crate::gpu::textures::TextureKind;
use crate::stats::Uploads;
use crate::{MeshId, Renderer};
use rusty_asset::MapRef;
use rusty_scene::serialize::SceneData;

/// One resource to make.
#[derive(Debug, PartialEq)]
enum Item {
    /// The geometry of entity `entities[index]`, the newest of its identity.
    Mesh(usize),
    /// A material map or the sky's panorama, as the slot reading it holds it.
    Texture(MapRef, TextureKind),
    /// The colour, direction and shadowmask page arrays.
    Lightmaps,
}

/// What is left to warm of one prepared scene.
pub struct Warming {
    items: Vec<Item>,
    next: usize,
    /// The thread reading the plan's textures, once a step needed one.
    decoding: Option<decode::Decoding>,
    /// What the steps so far uploaded.
    pub uploads: Uploads,
}

impl std::fmt::Debug for Warming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Warming")
            .field("items", &self.items)
            .field("next", &self.next)
            .field("uploads", &self.uploads)
            .finish()
    }
}

impl Warming {
    /// Everything `data` (a prepared scene's document) draws with, once each.
    pub fn plan(data: &SceneData) -> Self {
        let mut items = meshes(data);
        let mut seen = BTreeSet::new();
        let mut texture = |map: MapRef, kind: TextureKind| {
            if seen.insert((map.clone(), kind)) {
                items.push(Item::Texture(map, kind));
            }
        };
        let worn: BTreeSet<&str> = (data.entities.iter())
            .flat_map(|e| e.material.iter().flat_map(|m| m.materials.iter()))
            .map(String::as_str)
            .collect();
        for material in worn.into_iter().filter_map(|name| data.materials.get(name)) {
            for (map, kind) in material_maps(Some(material)) {
                texture(map, kind);
            }
        }
        if !data.skybox_path.is_empty() {
            // `load_panorama` loads the file as a colour texture.
            texture(MapRef::from(data.skybox_path.as_str()), TextureKind::Colour);
        }
        // Directions and shadowmasks are bound only beside colour pages.
        if !data.lightmaps.pages.is_empty() {
            items.push(Item::Lightmaps);
        }
        Self {
            items,
            next: 0,
            decoding: None,
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

/// One mesh item per geometry slot the extract would upload for `data` on a
/// renderer holding nothing (`stale_uploads`, its own rule), naming the entity
/// carrying the slot's newest revision.
fn meshes(data: &SceneData) -> Vec<Item> {
    let candidates = data.entities.iter().enumerate().filter_map(|(i, e)| {
        let mesh = e.mesh.as_ref()?;
        Some(UploadCandidate {
            id: MeshId::from_mesh(mesh),
            revision: mesh.revision,
            entity: i as u32,
            has_geometry: has_geometry(&mesh.vertices, &mesh.indices),
        })
    });
    let slots = stale_uploads(candidates, |_| None).into_values();
    slots.map(|(_, i)| Item::Mesh(i as usize)).collect()
}

impl Renderer {
    /// Make the next resource `warming` lists of `data`, the document it was planned
    /// from. Returns whether one was made; `false` once the plan is done, or while
    /// its next resource is still read on a worker (a texture, the lightmap pages),
    /// when the host tries again on a later frame. What a step uploads is the
    /// warm-up's, never counted in a frame's `asset_uploads`.
    pub fn warm_step(&mut self, data: &SceneData, warming: &mut Warming) -> bool {
        if warming.is_done() {
            return false;
        }
        let frame = std::mem::take(&mut self.uploads);
        let pages = std::mem::take(&mut self.lightmaps.uploads);
        let made = self.make(data, warming);
        warming
            .uploads
            .add_all(std::mem::replace(&mut self.uploads, frame));
        warming
            .uploads
            .add_all(std::mem::replace(&mut self.lightmaps.uploads, pages));
        warming.next += usize::from(made);
        made
    }

    /// Make `warming`'s next item; whether it is made.
    fn make(&mut self, data: &SceneData, warming: &mut Warming) -> bool {
        let index = warming.next;
        match &warming.items[index] {
            Item::Mesh(entity) => {
                let mesh = data.entities[*entity].mesh.as_ref().expect("planned");
                let id = MeshId::from_mesh(mesh);
                let resident = |id: &MeshId| self.gpu_meshes.get(id).map(|g| g.revision);
                let candidate = UploadCandidate {
                    id: id.clone(),
                    revision: mesh.revision,
                    entity: 0,
                    has_geometry: true,
                };
                if !stale_uploads([candidate], resident).is_empty() {
                    self.upload_geometry(id, mesh.revision, mesh);
                }
                true
            }
            Item::Texture(map, kind) => {
                let (map, kind) = (map.clone(), *kind);
                self.warm_texture(warming, index, &map, kind)
            }
            Item::Lightmaps => {
                let set = &data.lightmaps;
                let lists = [&set.pages[..], &set.directions[..], &set.masks[..]];
                self.lightmaps.warm(&self.device, &self.queue, lists)
            }
        }
    }

    /// Make the texture at `index` of `warming`'s plan: found by a lookup when it
    /// reads nothing from disk, else uploaded from its thread's read. Whether it is
    /// made (`false` while it is still being read).
    fn warm_texture(
        &mut self,
        warming: &mut Warming,
        index: usize,
        map: &MapRef,
        kind: TextureKind,
    ) -> bool {
        if self.texture_settled(map, kind) {
            self.load_texture_as(map, kind);
            return true;
        }
        let features = self.device.features();
        let decoding = match &mut warming.decoding {
            Some(decoding) => decoding,
            none => none.insert(decode::Decoding::start(
                self.unsettled(&warming.items[index..], index),
                features,
            )),
        };
        match decoding.take(index, map) {
            decode::Taken::Ready(decoded) => self.install_decoded(map, kind, decoded),
            decode::Taken::Waiting => return false,
            decode::Taken::Lost => drop(self.load_texture_as(map, kind)),
        }
        true
    }

    /// The textures of `items` (from plan index `from` on) a lookup would read from
    /// disk.
    fn unsettled(&self, items: &[Item], from: usize) -> Vec<decode::Job> {
        let textures = items.iter().enumerate().filter_map(|(i, item)| match item {
            Item::Texture(map, kind) => Some((from + i, map.clone(), *kind)),
            _ => None,
        });
        textures
            .filter(|(_, map, kind)| !self.texture_settled(map, *kind))
            .collect()
    }
}

#[cfg(test)]
#[path = "plan_tests.rs"]
mod plan_tests;
#[cfg(test)]
#[path = "prefab_gpu_tests.rs"]
mod prefab_gpu_tests;
#[cfg(test)]
#[path = "warm_gpu_tests.rs"]
mod warm_gpu_tests;
