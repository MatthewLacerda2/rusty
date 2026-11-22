//! The ray-traced scene: a two-level tree of each scene's static geometry, traced
//! in shader code (`rt/trace.wgsl`) on any adapter.

pub(crate) mod build;
mod gpu;
mod mesh;
#[cfg(any(test, feature = "dev"))]
pub mod probe;
mod top;
#[cfg(test)]
pub(crate) mod walk;

use std::collections::BTreeMap;

pub(crate) use gpu::layout;
#[cfg(any(test, feature = "dev"))]
pub(crate) use top::Packed;

use crate::debug_meshes::has_geometry;
use crate::scene::{MeshHandle, RenderObject, RenderScene};
use crate::Renderer;
use mesh::MeshTree;
use rusty_components::mesh::MeshRevision;
use rusty_scene::{Scene, SceneId};
use top::Placed;

/// The bind group a pass importing `rt::trace` sets at this index.
pub const GROUP: u32 = 3;

/// One scene's tree, kept from its retained objects: rebuilt only after a static
/// object changed (its `touch`) or the cameras' layers did.
#[derive(Default)]
pub struct RayTree {
    meshes: BTreeMap<MeshHandle, (MeshRevision, MeshTree)>,
    placed: Vec<Placed>,
    layers: Option<u32>,
    stale: bool,
    gpu: Option<gpu::GpuTree>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rebuilt {
    pub top: u32,
    pub meshes: u32,
}

impl RayTree {
    pub(crate) fn touch(&mut self) {
        self.stale = true;
    }

    #[cfg(any(test, feature = "dev"))]
    pub(crate) fn packed(&self) -> Packed {
        top::pack(self.meshes.iter().map(|(&h, (_, t))| (h, t)), &self.placed).0
    }

    #[cfg(any(test, feature = "dev"))]
    pub(crate) fn mesh_count(&self) -> u32 {
        self.meshes.len() as u32
    }

    pub fn gpu_bytes(&self) -> u64 {
        self.gpu.as_ref().map_or(0, gpu::GpuTree::bytes)
    }

    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.gpu.as_ref().map(|g| &g.group)
    }

    /// True when the top changed.
    fn rebuild(&mut self, rs: &RenderScene, scene: &Scene, built: &mut Rebuilt) -> bool {
        let layers = camera_layers(scene);
        if !self.stale && self.layers == Some(layers) {
            return false;
        }
        let traced = rs.objects().filter(|(_, o)| traced(o, layers));
        let traced: Vec<(u32, &RenderObject)> = traced.collect();
        let mut kept = BTreeMap::new();
        for &(id, o) in &traced {
            if kept.contains_key(&o.mesh) {
                continue;
            }
            let mesh = scene.world.mesh(id);
            let Some(mesh) = mesh.filter(|m| has_geometry(&m.vertices, &m.indices)) else {
                continue;
            };
            let tree = match self.meshes.remove(&o.mesh) {
                Some((rev, tree)) if rev == mesh.revision => tree,
                _ => {
                    built.meshes += 1;
                    MeshTree::build(&mesh.vertices, &mesh.indices, mesh.index_ranges())
                }
            };
            kept.insert(o.mesh, (mesh.revision, tree));
        }
        let gone = !self.meshes.is_empty();
        self.meshes = kept;
        self.placed = traced.iter().filter_map(|(_, o)| Placed::of(o)).collect();
        self.layers = Some(layers);
        self.stale = false;
        built.top += 1;
        if gone || built.meshes > 0 {
            self.gpu = None;
        }
        true
    }
}

fn traced(o: &RenderObject, layers: u32) -> bool {
    let layer = rusty_scene::layer_in_mask(o.layer, layers);
    o.is_static && o.shown && o.resident && !o.palette && !o.moving && layer
}

/// The layers some camera of `scene` draws: every layer when it has none.
fn camera_layers(scene: &Scene) -> u32 {
    let cameras = scene.world.ids_with_camera().into_iter();
    let masks = cameras.filter_map(|id| scene.world.camera(id).map(|c| c.culling_mask));
    masks.reduce(|a, b| a | b).unwrap_or(u32::MAX)
}

impl Renderer {
    /// `scene`'s ray-traced tree, built or refreshed now; `None` before its first extract.
    pub fn ray_tree(&mut self, scene: &Scene) -> Option<&RayTree> {
        self.refresh_rays(scene.id(), scene)?;
        self.retained.scene(scene.id()).map(|rs| &rs.rays)
    }

    fn refresh_rays(&mut self, id: SceneId, scene: &Scene) -> Option<()> {
        let rs = self.retained.scene_mut(id)?;
        let mut tree = std::mem::take(&mut rs.rays);
        let mut built = Rebuilt::default();
        let moved = tree.rebuild(rs, scene, &mut built);
        if moved || tree.gpu.is_none() {
            let layout = self.retained.ray_layout(&self.device);
            let (packed, geometry) =
                top::pack(tree.meshes.iter().map(|(&h, (_, t))| (h, t)), &tree.placed);
            let gpu = (&self.device, &self.queue, layout);
            tree.gpu = Some(gpu::GpuTree::upload(
                gpu,
                &packed,
                geometry,
                tree.gpu.take(),
            ));
        }
        self.frame_counters.ray_tree_rebuilds += built.top;
        self.frame_counters.ray_mesh_builds += built.meshes;
        self.retained.scene_mut(id)?.rays = tree;
        Some(())
    }

    #[cfg(test)]
    pub(crate) fn rays_kept_equal_fresh(&mut self, scene: &Scene) -> Result<(), String> {
        let kept = self.ray_tree(scene).ok_or("not extracted")?.packed();
        let rs = self.retained.scene(scene.id()).ok_or("not extracted")?;
        let mut fresh = RayTree::default();
        fresh.rebuild(rs, scene, &mut Rebuilt::default());
        let fresh = fresh.packed();
        match kept == fresh {
            true => Ok(()),
            false => Err(format!(
                "the kept ray tree differs from a fresh one: {:?} vs {:?}",
                kept.instances, fresh.instances
            )),
        }
    }

    pub(crate) fn ray_tree_bytes(&self) -> u64 {
        self.retained.scenes().map(|rs| rs.rays.gpu_bytes()).sum()
    }
}

#[cfg(all(test, feature = "dev"))]
mod dust2_gpu_tests;
#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod kept_gpu_tests;
#[cfg(test)]
mod trace_gpu_tests;
#[cfg(test)]
mod walk_tests;
