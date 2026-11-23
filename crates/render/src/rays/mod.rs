//! Ray-traced static geometry: a two-level tree traced in `rt/trace.wgsl` on any adapter.

mod ahead;
pub(crate) mod build;
mod frame;
mod gpu;
mod mesh;
#[cfg(any(test, feature = "dev"))]
pub mod probe;
mod top;
#[cfg(any(test, feature = "dev"))]
pub(crate) mod walk;

use std::collections::BTreeMap;

use ahead::Ahead;
pub(crate) use ahead::{prebuild, Prebuilt};
#[cfg(test)]
pub(crate) use frame::limit_for_tests;
pub(crate) use gpu::layout;
#[cfg(any(test, feature = "dev"))]
pub(crate) use top::Packed;

use crate::scene::{MeshHandle, RenderObject, RenderScene};
use crate::Renderer;
use mesh::MeshTree;
use rusty_components::mesh::MeshRevision;
use rusty_scene::{Scene, SceneId};
use top::Placed;

/// `rt::trace`'s bind group index.
pub const GROUP: u32 = 3;

/// Rebuilt after a static object's `touch` moved its placement or the active cameras'
/// layers change. A mesh's tree lives while its `GpuMesh` does, at its revision.
#[derive(Default)]
pub struct RayTree {
    meshes: BTreeMap<MeshHandle, (MeshRevision, MeshTree)>,
    placed: Vec<Placed>,
    layers: Option<u32>,
    stale: bool,
    refused: Option<gpu::Refused>,
    gpu: Option<gpu::GpuTree>,
    /// Kept until released (`frame.rs`).
    held: bool,
    fallback_said: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Rebuilt {
    pub top: u32,
    pub meshes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Changed {
    Top,
    Geometry,
}

/// `None` once the mesh was released.
type Resident<'a> = &'a dyn Fn(MeshHandle) -> Option<MeshRevision>;

impl RayTree {
    pub(crate) fn touch(&mut self) {
        self.stale = true;
    }

    /// As on the GPU, every kept mesh included.
    #[cfg(any(test, feature = "dev"))]
    pub(crate) fn packed(&self) -> Packed {
        let meshes = self.meshes.iter().map(|(&h, (_, t))| (h, t));
        top::pack(meshes, &self.placed, true).0
    }

    /// Only placed meshes: what a fresh build holds.
    #[cfg(test)]
    fn placed_only(&self) -> Packed {
        let placed = |h: &MeshHandle| self.placed.iter().any(|p| p.mesh == *h);
        let meshes = self.meshes.iter().filter(|(h, _)| placed(h));
        top::pack(meshes.map(|(&h, (_, t))| (h, t)), &self.placed, true).0
    }

    #[cfg(any(test, feature = "dev"))]
    pub(crate) fn mesh_count(&self) -> u32 {
        self.meshes.len() as u32
    }

    /// 0 while refused.
    pub fn gpu_bytes(&self) -> u64 {
        match self.refused {
            Some(_) => 0,
            None => self.gpu.as_ref().map_or(0, gpu::GpuTree::bytes),
        }
    }

    pub fn refused(&self) -> bool {
        self.refused.is_some()
    }

    pub fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.gpu.as_ref().map(|g| &g.group)
    }

    fn rebuild(
        &mut self,
        rs: &RenderScene,
        scene: &Scene,
        (resident, ahead): (Resident, Ahead),
        built: &mut Rebuilt,
    ) -> Option<Changed> {
        let layers = camera_layers(scene);
        let before = self.meshes.len();
        self.meshes.retain(|&h, (rev, _)| resident(h) == Some(*rev));
        let mut geometry = self.meshes.len() != before;
        if !geometry && !self.stale && self.layers == Some(layers) {
            return None;
        }
        let mut placed = Vec::new();
        for (id, o) in rs.objects().filter(|(_, o)| traced(o, layers)) {
            let Some(p) = Placed::of(o) else {
                continue;
            };
            if let Some(fresh) = self.keep_mesh((id, o.mesh), scene, (resident, &mut *ahead)) {
                built.meshes += u32::from(fresh);
                geometry = true;
            }
            placed.push(p);
        }
        let same = placed == self.placed && self.layers == Some(layers);
        self.stale = false;
        if !geometry && same && self.gpu.is_some() {
            return None;
        }
        self.placed = placed;
        self.layers = Some(layers);
        self.stale = false;
        built.top += 1;
        Some(if geometry {
            Changed::Geometry
        } else {
            Changed::Top
        })
    }
}

fn traced(o: &RenderObject, layers: u32) -> bool {
    let layer = rusty_scene::layer_in_mask(o.layer, layers);
    o.is_static && o.shown && o.resident && !o.palette && !o.moving && layer
}

/// The layers active cameras draw; every layer with none.
fn camera_layers(scene: &Scene) -> u32 {
    let world = &scene.world;
    let cameras = world.ids_with_camera().into_iter();
    let active = cameras.filter(|&id| world.is_active_in_hierarchy(id));
    let masks = active.filter_map(|id| {
        world
            .camera(id)
            .filter(|c| c.active)
            .map(|c| c.culling_mask)
    });
    masks.reduce(|a, b| a | b).unwrap_or(u32::MAX)
}

impl Renderer {
    /// `None` before the scene's first extract.
    pub fn ray_tree(&mut self, scene: &Scene) -> Option<&RayTree> {
        self.refresh_rays(scene.id(), scene)?;
        self.retained.scene(scene.id()).map(|rs| &rs.rays)
    }

    fn refresh_rays(&mut self, id: SceneId, scene: &Scene) -> Option<()> {
        if self.retained.scene(id)?.rays.held {
            return Some(());
        }
        let mut tree = std::mem::take(&mut self.retained.scene_mut(id)?.rays);
        let mut built = Rebuilt::default();
        let mut prebuilt = std::mem::take(&mut self.retained.ray_meshes);
        let rs = self.retained.scene(id)?;
        let resident = |h| self.gpu_mesh_of(h).map(|g| g.revision);
        let meshes = &self.retained.meshes;
        let mut ahead = |h, rev| prebuilt.remove(&(meshes.id(h).clone(), rev));
        let changed = tree.rebuild(rs, scene, (&resident, &mut ahead), &mut built);
        self.retained.ray_meshes = prebuilt;
        if changed.is_some() || tree.gpu.is_none() {
            self.upload_rays(&mut tree, changed == Some(Changed::Top), id);
        }
        self.frame_counters.ray_tree_rebuilds += built.top;
        self.frame_counters.ray_mesh_builds += built.meshes;
        self.retained.scene_mut(id)?.rays = tree;
        Some(())
    }

    /// Only the top when `top_only` and it fits; an empty tree when refused.
    fn upload_rays(&self, tree: &mut RayTree, top_only: bool, id: SceneId) {
        let layout = self.retained.ray_layout(&self.device);
        let gpu = (&self.device, &self.queue, layout);
        let meshes = || tree.meshes.iter().map(|(&h, (_, t))| (h, t));
        let kept = tree
            .gpu
            .take()
            .filter(|_| top_only && tree.refused.is_none());
        let kept = kept.and_then(|k| {
            let (top, geometry) = top::pack(meshes(), &tree.placed, false);
            k.top(gpu, &top, geometry)
        });
        let uploaded = kept.map_or_else(
            || gpu::GpuTree::whole(gpu, &top::pack(meshes(), &tree.placed, true).0),
            Ok,
        );
        match uploaded {
            Ok(uploaded) => {
                tree.refused = None;
                tree.gpu = Some(uploaded);
            }
            Err(refused) => {
                if tree.refused.is_none() {
                    log::warn!(
                        "[Render] the ray-traced tree of scene {id:?} is not uploaded: its {} buffer \
                         would take {} bytes, past this GPU's limit of {}; rays miss \
                         everything in the scene until it fits",
                        refused.buffer,
                        refused.bytes,
                        refused.limit
                    );
                }
                tree.refused = Some(refused);
                tree.gpu = Some(gpu::GpuTree::empty(gpu));
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn rays_kept_equal_fresh(&mut self, scene: &Scene) -> Result<(), String> {
        let kept = self.ray_tree(scene).ok_or("not extracted")?.placed_only();
        let rs = self.retained.scene(scene.id()).ok_or("not extracted")?;
        let mut fresh = RayTree::default();
        let resident = |h| self.gpu_mesh_of(h).map(|g| g.revision);
        let ahead: Ahead = &mut |_, _| None;
        fresh.rebuild(rs, scene, (&resident, ahead), &mut Rebuilt::default());
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
mod frame_gpu_tests;
#[cfg(test)]
mod kept_gpu_tests;
#[cfg(test)]
mod limit_gpu_tests;
#[cfg(test)]
mod trace_gpu_tests;
#[cfg(test)]
mod walk_tests;
