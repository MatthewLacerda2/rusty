//! A scene load's GPU warm-up: what a prepared scene draws with, one whole resource a step, while
//! the old scene still draws.

mod decode;

use std::collections::BTreeSet;
use std::sync::Arc;

use crate::debug_meshes::{has_geometry, stale_uploads, UploadCandidate};
use crate::draw::map_slots::material_maps;
use crate::gpu::textures::TextureKind;
use crate::stats::Uploads;
use crate::{MeshId, Renderer};
use rusty_asset::MapRef;
use rusty_scene::serialize::SceneData;

#[derive(Debug, PartialEq)]
enum Item {
    Mesh(usize),
    Texture(MapRef, TextureKind),
    Lightmaps,
    /// A static mesh's ray tree, when SSR traces rays (*Mixed*): its first traced frame
    /// takes it instead of building it.
    RayMesh(usize),
}

pub struct Warming {
    items: Vec<Item>,
    next: usize,
    decoding: Option<decode::Decoding>,
    pub uploads: Uploads,
    token: Arc<()>,
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
            texture(MapRef::from(data.skybox_path.as_str()), TextureKind::Colour);
        }
        if !data.lightmaps.pages.is_empty() {
            items.push(Item::Lightmaps);
        }
        items.extend(ray_meshes(data));
        Self {
            items,
            next: 0,
            decoding: None,
            uploads: Uploads::default(),
            token: Arc::new(()),
        }
    }

    pub fn progress(&self) -> f32 {
        match self.items.len() {
            0 => 1.0,
            n => self.next as f32 / n as f32,
        }
    }

    pub fn is_done(&self) -> bool {
        self.next >= self.items.len()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Whether a volume in `data` traces SSR's rays.
fn traces_rays(data: &SceneData) -> bool {
    let volumes = data
        .entities
        .iter()
        .filter_map(|e| e.post_processing.as_ref());
    volumes
        .into_iter()
        .any(|v| v.ssr.active && v.ssr.tracing == rusty_components::SsrTracing::Mixed)
}

fn ray_meshes(data: &SceneData) -> Vec<Item> {
    if !traces_rays(data) {
        return Vec::new();
    }
    let mut seen = BTreeSet::new();
    let statics = data
        .entities
        .iter()
        .enumerate()
        .filter(|(_, e)| e.is_static);
    let meshes = statics.filter_map(|(i, e)| Some((i, e.mesh.as_ref()?)));
    meshes
        .filter(|(_, m)| has_geometry(&m.vertices, &m.indices))
        .filter(|(_, m)| seen.insert((MeshId::from_mesh(m), m.revision)))
        .map(|(i, _)| Item::RayMesh(i))
        .collect()
}

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
    /// Makes the next resource; returns whether it did: `false` when done, or while the next is
    /// still read on a worker (retry next frame).
    pub fn warm_step(&mut self, data: &SceneData, warming: &mut Warming) -> bool {
        if warming.is_done() {
            return false;
        }
        if warming.next == 0 {
            self.retained.ray_meshes.clear(); // an earlier load's, never taken
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
                    self.upload_geometry(id.clone(), mesh.revision, mesh);
                }
                self.retained.residency.warmed(&warming.token, id);
                true
            }
            Item::Texture(map, kind) => {
                let (map, kind) = (map.clone(), *kind);
                self.retained.residency.warmed_map(&warming.token, &map);
                self.warm_texture(warming, index, &map, kind)
            }
            Item::RayMesh(entity) => {
                let mesh = data.entities[*entity].mesh.as_ref().expect("planned");
                let key = (MeshId::from_mesh(mesh), mesh.revision);
                if let Some(tree) = crate::rays::prebuild(mesh) {
                    self.retained.ray_meshes.insert(key, tree);
                }
                true
            }
            Item::Lightmaps => {
                let set = &data.lightmaps;
                let lists = [&set.pages[..], &set.directions[..], &set.masks[..]];
                (self.retained.residency).warmed_lightmaps(&warming.token, lists);
                (self.lightmaps).warm(&self.device, (&self.queue, &self.staging), lists)
            }
        }
    }

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
