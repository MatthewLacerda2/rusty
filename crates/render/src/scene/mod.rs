mod casters;
mod classes;
#[cfg(test)]
mod classes_gpu_tests;
mod draw;
#[cfg(test)]
mod equivalence_ops;
#[cfg(test)]
mod equivalence_tests;
mod extract;
#[cfg(test)]
mod fixture;
#[cfg(test)]
mod gpu_tests;
mod handles;
#[cfg(test)]
mod idle_tests;
mod inputs;
#[cfg(test)]
mod interpolate_gpu_tests;
#[cfg(test)]
mod library_gpu_tests;
mod object;
mod occlusion;
#[cfg(test)]
mod priority_gpu_tests;
pub(crate) mod records;
#[cfg(test)]
mod records_gpu_tests;
pub(crate) mod residency;
#[cfg(test)]
mod shadow_ops;
#[cfg(test)]
mod stack_blend_gpu_tests;
#[cfg(test)]
mod stack_gpu_tests;
mod table;

use std::collections::{BTreeMap, BTreeSet};

pub(crate) use casters::CasterIndex;
pub(crate) use draw::CameraSolids;
pub(crate) use handles::{MeshHandle, MeshHandles};
pub(crate) use object::{RenderObject, SlotDraw};
#[cfg(test)]
pub(crate) use occlusion::fixture as occlusion_fixture;
pub(crate) use occlusion::OcclusionBinding;
pub(crate) use records::{MaterialRecords, Variant};
pub(crate) use table::{ObjectTable, SlotTable, Slotted};

use crate::passes::shadows::UnbakedCasters;
use crate::{GpuMesh, Renderer};
use rusty_scene::{Scene, SceneId};

/// Scenes kept at once (the main one, a preview tab, a capture's); past it the least recently drawn
/// goes.
const MAX_SCENES: usize = 4;

#[derive(Default)]
pub(crate) struct Retained {
    pub meshes: MeshHandles,
    pub objects: ObjectTable,
    pub records: MaterialRecords,
    inputs: u64,
    pub render_textures: u64,
    scenes: Vec<(SceneId, RenderScene)>,
    pub residency: residency::Residency,
    ray_layout: std::cell::OnceCell<wgpu::BindGroupLayout>,
}

impl Retained {
    pub(crate) fn touch(&mut self) {
        self.inputs += 1;
    }

    fn take(&mut self, id: SceneId) -> RenderScene {
        match self.scenes.iter().position(|(s, _)| *s == id) {
            Some(at) => self.scenes.remove(at).1,
            None => RenderScene::default(),
        }
    }

    fn put(&mut self, id: SceneId, scene: RenderScene) -> Option<SceneId> {
        self.scenes.push((id, scene));
        let gone = (self.scenes.len() > MAX_SCENES).then(|| self.scenes[0].0)?;
        self.drop_scene(gone);
        Some(gone)
    }

    fn drop_scene(&mut self, id: SceneId) -> bool {
        let Some(at) = self.scenes.iter().position(|(s, _)| *s == id) else {
            return false;
        };
        self.scenes.remove(at).1.release(&mut self.objects);
        self.records.release_scene(id);
        true
    }

    #[cfg(test)]
    pub(crate) fn kept_objects(&self) -> usize {
        self.scenes.iter().map(|(_, s)| s.objects.len()).sum()
    }

    pub(crate) fn scene(&self, id: SceneId) -> Option<&RenderScene> {
        self.scenes.iter().find(|(s, _)| *s == id).map(|(_, r)| r)
    }

    pub(crate) fn scene_mut(&mut self, id: SceneId) -> Option<&mut RenderScene> {
        self.scenes
            .iter_mut()
            .find(|(s, _)| *s == id)
            .map(|(_, r)| r)
    }

    pub(crate) fn scenes(&self) -> impl Iterator<Item = &RenderScene> {
        self.scenes.iter().map(|(_, r)| r)
    }

    pub(crate) fn scenes_mut(&mut self) -> impl Iterator<Item = &mut RenderScene> {
        self.scenes.iter_mut().map(|(_, r)| r)
    }

    pub(crate) fn ray_layout(&self, device: &wgpu::Device) -> &wgpu::BindGroupLayout {
        self.ray_layout.get_or_init(|| crate::rays::layout(device))
    }
}

#[derive(Default)]
pub(crate) struct RenderScene {
    inputs: Option<inputs::Inputs>,
    objects: BTreeMap<u32, RenderObject>,
    /// Ids by insertion sequence: `ids_with_mesh`'s order.
    order: BTreeMap<u64, u32>,
    by_material: BTreeMap<String, BTreeSet<u32>>,
    pub(crate) unbaked: UnbakedCasters,
    casting: Option<u64>,
    pub(crate) occlusion: OcclusionBinding,
    casters: casters::Gathering,
    library_meshes: Vec<String>,
    pub(crate) rays: crate::rays::RayTree,
}

impl RenderScene {
    /// The objects in `ids_with_mesh` order, with their ids.
    pub(crate) fn objects(&self) -> impl Iterator<Item = (u32, &RenderObject)> {
        self.order.values().map(|&id| (id, &self.objects[&id]))
    }

    fn set(&mut self, id: u32, object: Option<RenderObject>) {
        if let Some(old) = self.objects.remove(&id) {
            self.order.remove(&old.seq);
            self.forget_casting(id, old.is_static);
            for name in &old.materials {
                let users = self.by_material.get_mut(name).expect("indexed with it");
                users.remove(&id);
                if users.is_empty() {
                    self.by_material.remove(name);
                }
            }
        }
        if let Some(object) = object {
            self.forget_casting(id, object.is_static);
            self.order.insert(object.seq, id);
            for name in &object.materials {
                self.by_material.entry(name.clone()).or_default().insert(id);
            }
            self.objects.insert(id, object);
        }
        self.casters.set(id, self.objects.get(&id));
    }

    fn users_of<'a>(&'a self, names: &'a BTreeSet<String>) -> impl Iterator<Item = u32> + 'a {
        names
            .iter()
            .filter_map(|name| self.by_material.get(name))
            .flatten()
            .copied()
    }

    fn users_of_name<'a>(&'a self, name: &str) -> impl Iterator<Item = u32> + 'a {
        self.by_material.get(name).into_iter().flatten().copied()
    }

    pub(super) fn release(&mut self, table: &mut ObjectTable) {
        for object in self.objects.values() {
            table.release(object.slot);
        }
        self.objects.clear();
        self.order.clear();
        self.by_material.clear();
        self.casting = None;
        self.occlusion.forget();
        self.casters = casters::Gathering::default();
        self.rays = crate::rays::RayTree::default();
    }

    fn forget_casting(&mut self, id: u32, is_static: bool) {
        if is_static {
            self.casting = None;
            self.occlusion.touch(id);
            self.casters.touch(id);
            self.rays.touch();
        }
    }

    fn regroup(&mut self, scene: &Scene, unbaked: UnbakedCasters) {
        self.occlusion.regroup(scene);
        if unbaked != self.unbaked {
            self.unbaked = unbaked;
            self.casting = None;
            self.casters.stale();
        }
    }
}

impl Renderer {
    pub(crate) fn gpu_mesh_of(&self, handle: MeshHandle) -> Option<&GpuMesh> {
        self.gpu_meshes.get(self.retained.meshes.id(handle))
    }
}
