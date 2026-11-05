//! The frame's `Baked` point and spot lamps (#1703), for the volumetric fog alone.
//! A `Baked` lamp reaches surfaces only through the lightmaps and probes, so
//! [`super::local_lights`] leaves it out of the frame's lights; the air has no
//! lightmap, so the froxels light it from this list instead, unshadowed (a
//! shadowmask could bound it later, #1695).
//!
//! **Kept, not walked.** The list is rebuilt from the scene's lights only when a
//! `Baked` lamp (or an entity listed as one) changed its light, world pose or
//! activity (`ecs::changes` cursors, as the retained scene keeps its objects,
//! #1706) or another scene is drawn; a frame that changed none of them uploads
//! nothing, and a live light moving or flickering every frame rebuilds nothing.
//! Area lights are not lamps here: a panel gives no halo yet. Each camera bins it into its own
//! clusters with the lights' budget ([`super::bin`]: the 64 that contribute most
//! to the view, 32 per cluster), so a froxel loops over at most 32 of them however
//! many the level holds.

use std::collections::BTreeSet;

use glam::Vec3;

use super::bin::bin;
use super::grid::AabbCache;
use super::{ClusterGrid, LocalLight};
use crate::components::{LightComponent, LightType};
use crate::ecs::{Activity, Changes, Cursor, World, WorldPose};
use crate::render::gpu::grow_buffer::GrowBuffer;
use crate::scene::revision::Revision;
use crate::scene::{Scene, SceneId};

/// The change cursors the list is kept by, and the scene they read.
struct Kept {
    scene: SceneId,
    /// The bake's record (#1695): a rebake can turn a `Mixed` light `Baked`.
    lightmaps: Revision,
    light: Cursor<LightComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
}

/// The `Baked` lamps' records and one camera's bins of them, on the GPU.
pub(crate) struct BakedLamps {
    kept: Option<Kept>,
    /// The entities listed, so a pose or activity change on one is seen even
    /// after it stopped being a light.
    listed: BTreeSet<u32>,
    lamps: Vec<LocalLight>,
    lights: GrowBuffer,
    ranges: GrowBuffer,
    indices: GrowBuffer,
    aabbs: AabbCache,
    /// How many times the list was rebuilt: what a test counts a kept frame by.
    pub(crate) rebuilds: u64,
}

impl BakedLamps {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let storage = |label| GrowBuffer::new(device, label, wgpu::BufferUsages::STORAGE);
        Self {
            kept: None,
            listed: BTreeSet::new(),
            lamps: Vec::new(),
            lights: storage("Baked Lamps"),
            ranges: storage("Baked Lamp Ranges"),
            indices: storage("Baked Lamp Indices"),
            aabbs: AabbCache::default(),
            rebuilds: 0,
        }
    }

    /// The records, each cluster's `[offset, count]` and the index list.
    pub(crate) fn buffers(&self) -> [&wgpu::Buffer; 3] {
        [
            self.lights.buffer(),
            self.ranges.buffer(),
            self.indices.buffer(),
        ]
    }

    /// The lamps listed now.
    #[cfg(test)]
    pub(crate) fn lamps(&self) -> &[LocalLight] {
        &self.lamps
    }

    /// Bring the list up to date with `scene`, rebuilding it only when a light,
    /// pose or activity it depends on changed. Returns whether a buffer was
    /// replaced (a bind group naming it is stale).
    pub(crate) fn refresh(&mut self, scene: &Scene, gpu: (&wgpu::Device, &wgpu::Queue)) -> bool {
        if !self.changed(scene) {
            return false;
        }
        self.lamps = baked_lamps(scene);
        self.listed = baked_ids(scene).collect();
        self.rebuilds += 1;
        self.lights
            .upload(gpu.0, gpu.1, bytemuck::cast_slice(&self.lamps))
    }

    /// Whether the scene's lights may differ from the list: another scene, a
    /// rebuild the log asks for, or a changed id that is or was a light.
    fn changed(&mut self, scene: &Scene) -> bool {
        let world = &scene.world;
        let rebaked = |k: &Kept| k.lightmaps != scene.lightmaps.revision();
        let Some(kept) = self
            .kept
            .as_mut()
            .filter(|k| k.scene == scene.id() && !rebaked(k))
        else {
            self.kept = Some(Kept {
                scene: scene.id(),
                lightmaps: scene.lightmaps.revision(),
                light: world.change_cursor(),
                pose: world.change_cursor(),
                activity: world.change_cursor(),
            });
            return true;
        };
        // Only an id that is or was a listed lamp: a live light moving or
        // flickering every frame (a beacon, a flashlight) rebuilds nothing.
        let touches = |c: Changes, listed: &BTreeSet<u32>| match c {
            Changes::Rebuild => true,
            Changes::Ids(ids) => ids
                .iter()
                .any(|&id| listed.contains(&id) || is_baked_lamp(scene, id)),
        };
        let light = touches(world.changes(&mut kept.light), &self.listed);
        let pose = touches(world.changes(&mut kept.pose), &self.listed);
        let active = touches(world.changes(&mut kept.activity), &self.listed);
        light | pose | active
    }

    /// Bin the list into `grid`'s clusters and upload the bins. Returns whether a
    /// buffer was replaced.
    pub(crate) fn bin(&mut self, grid: &ClusterGrid, gpu: (&wgpu::Device, &wgpu::Queue)) -> bool {
        let binned = bin(grid, &self.lamps, &mut self.aabbs);
        let (device, queue) = gpu;
        let ranges = bytemuck::cast_slice(&binned.ranges);
        let mut grown = self.ranges.upload(device, queue, ranges);
        // An empty list still binds a buffer the shader may index: one zero entry.
        let indices = if binned.indices.is_empty() {
            vec![0]
        } else {
            binned.indices
        };
        grown |= self
            .indices
            .upload(device, queue, bytemuck::cast_slice(&indices));
        grown
    }
}

/// Whether `id` holds a `Baked` point or spot light (`LightMode::renders_live` is
/// false for it), active or not. Area lights are left out: always `Baked`, they
/// would enter as an unshadowed far-field stand-in of `intensity × area`, and a
/// panel's light in the air is not a lamp's halo; a panel gives no halo yet.
/// A `Mixed` light the last bake baked as `Baked` (#1695, `Scene::light_mode`)
/// is one too.
fn is_baked_lamp(scene: &Scene, id: u32) -> bool {
    scene.world.light(id).is_some_and(|l| {
        matches!(l.light_type, LightType::Point | LightType::Spotlight)
            && !scene.light_mode(id, &l).renders_live(false)
    })
}

/// The active `Baked` point and spot lights of `scene`, in entity order.
fn baked_ids(scene: &Scene) -> impl Iterator<Item = u32> + '_ {
    let world: &World = &scene.world;
    world
        .ids_with_light()
        .into_iter()
        .filter(move |&id| world.is_active_in_hierarchy(id) && is_baked_lamp(scene, id))
}

/// [`baked_ids`]' records, unshadowed.
fn baked_lamps(scene: &Scene) -> Vec<LocalLight> {
    let world = &scene.world;
    baked_ids(scene)
        .filter_map(|id| {
            let light = world.light(id)?;
            let at = world.world_position(id);
            let forward = world.transform_direction(id, Vec3::NEG_Z);
            LocalLight::new(at, forward, &light)
        })
        .collect()
}
