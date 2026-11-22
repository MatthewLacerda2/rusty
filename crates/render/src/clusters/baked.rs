use std::collections::BTreeSet;

use glam::Vec3;

use super::bin::bin;
use super::grid::AabbCache;
use super::{ClusterGrid, LocalLight};
use crate::gpu::grow_buffer::GrowBuffer;
use rusty_components::{LightComponent, LightType};
use rusty_ecs::{Activity, Changes, Cursor, World, WorldPose};
use rusty_scene::revision::Revision;
use rusty_scene::{Scene, SceneId};

struct Kept {
    scene: SceneId,
    lightmaps: Revision,
    light: Cursor<LightComponent>,
    pose: Cursor<WorldPose>,
    activity: Cursor<Activity>,
}

pub(crate) struct BakedLamps {
    kept: Option<Kept>,
    listed: BTreeSet<u32>,
    lamps: Vec<LocalLight>,
    lights: GrowBuffer,
    ranges: GrowBuffer,
    indices: GrowBuffer,
    aabbs: AabbCache,
    /// Rebuilds so far. A test hook: `volumetric_fog::gpu::kept` counts kept frames by it.
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

    pub(crate) fn buffers(&self) -> [&wgpu::Buffer; 3] {
        [
            self.lights.buffer(),
            self.ranges.buffer(),
            self.indices.buffer(),
        ]
    }

    #[cfg(test)]
    pub(crate) fn lamps(&self) -> &[LocalLight] {
        &self.lamps
    }

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

    pub(crate) fn bin(&mut self, grid: &ClusterGrid, gpu: (&wgpu::Device, &wgpu::Queue)) -> bool {
        let binned = bin(grid, &self.lamps, &mut self.aabbs);
        let (device, queue) = gpu;
        let ranges = bytemuck::cast_slice(&binned.ranges);
        let mut grown = self.ranges.upload(device, queue, ranges);
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

fn is_baked_lamp(scene: &Scene, id: u32) -> bool {
    scene.world.light(id).is_some_and(|l| {
        matches!(l.light_type, LightType::Point | LightType::Spotlight)
            && !crate::clusters::drawn_mode(scene, id, &l).renders_live(false)
    })
}

fn baked_ids(scene: &Scene) -> impl Iterator<Item = u32> + '_ {
    let world: &World = &scene.world;
    world
        .ids_with_light()
        .into_iter()
        .filter(move |&id| world.is_active_in_hierarchy(id) && is_baked_lamp(scene, id))
}

fn baked_lamps(scene: &Scene) -> Vec<LocalLight> {
    let world = &scene.world;
    baked_ids(scene)
        .filter_map(|id| {
            let light = world.light(id)?;
            let at = world.world_position(id);
            let forward = world.transform_direction(id, Vec3::NEG_Z);
            let lamp = LocalLight::new(at, forward, &light)?;
            Some(LocalLight { baked: 1.0, ..lamp })
        })
        .collect()
}
