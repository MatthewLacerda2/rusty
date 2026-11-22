use glam::Vec3;

use super::{ProbeArray, MAX_REFLECTION_PROBES, SPARE_SLOTS};
use crate::ibl::cubemap::{read_cube_shape, CubeShape, Refusal};
use crate::{RenderSwitch, Renderer};
use rusty_scene::{ReflectionProbe, Scene};

impl Renderer {
    pub(crate) fn update_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) {
        while !self.place_reflection_probes(scene, camera_pos) {}
    }

    fn place_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) -> bool {
        let probes = &scene.reflection_probes.probes;
        let probes = self.switches.listed(RenderSwitch::ReflectionProbes, probes);
        let budget = scene.lighting_settings.reflections.budget_bytes();
        let format = self.reflection_probes.format();
        let mut wanted: Vec<(usize, CubeShape)> = Vec::new();
        let (mut face, mut paths) = (0, Vec::new());
        for i in ranked(probes, camera_pos) {
            if wanted.len() == MAX_REFLECTION_PROBES {
                break;
            }
            let path = probes[i].cubemap_path.as_str();
            let cubes = paths.len() + usize::from(!paths.contains(&path)) + SPARE_SLOTS;
            let exempt = wanted.iter().all(|&(j, _)| probes[j].fallback);
            let over = |size: u32| !exempt && array_bytes(face.max(size), cubes, format) > budget;
            if over(0) {
                continue; // full: no cube is read
            }
            let Some(shape) = self.reflection_probes.shape(path) else {
                continue;
            };
            if over(shape.size) {
                continue;
            }
            face = face.max(shape.size);
            paths = push_new(paths, path);
            wanted.push((i, shape));
        }
        wanted.sort_by_key(|&(i, _)| (!probes[i].fallback, i));
        let wanted: Vec<_> = wanted.into_iter().map(|(i, s)| (&probes[i], s)).collect();
        let (copies, rebuilt) = self.reflection_probes.place(&self.device, &wanted, budget);
        let (mut loaded, mut placed) = (Vec::new(), true);
        for (path, shape, slot, shift) in copies {
            #[cfg(test)]
            {
                self.reflection_probes.loads += 1;
            }
            match self.load_cubemap_mips(&path, shape) {
                Ok(cube) => loaded.push((cube, slot, shift)),
                Err(why) => {
                    self.reflection_probes.refuse(&path, why);
                    placed = false;
                }
            }
        }
        (self.reflection_probes).fill(&self.device, &self.queue, &loaded);
        self.global_bind_group_dirty |= rebuilt;
        placed
    }
}

impl ProbeArray {
    fn shape(&mut self, path: &str) -> Option<CubeShape> {
        if let Some(&known) = self.shapes.get(path) {
            return known;
        }
        match read_cube_shape(path) {
            Ok(shape) if Self::admits(path, shape.size) => {
                self.shapes.insert(path.to_string(), Some(shape));
                Some(shape)
            }
            Ok(_) => {
                self.shapes.insert(path.to_string(), None);
                None
            }
            Err(why) => {
                self.refuse(path, why);
                None
            }
        }
    }

    fn refuse(&mut self, path: &str, why: Refusal) {
        log::warn!("[Reflection] cubemap {path} is left out: {why}");
        self.table.forget(|p| p == path);
        self.shapes.insert(path.to_string(), None);
    }
}

pub(crate) fn array_bytes(face: u32, cubes: usize, format: wgpu::TextureFormat) -> u64 {
    let (edge, _) = format.block_dimensions();
    let block = u64::from(format.block_copy_size(None).unwrap_or(8));
    let blocks = u64::from(face.div_ceil(edge));
    let level = blocks * blocks * block * 6;
    level * 4 / 3 * cubes as u64
}

fn push_new<'a>(mut paths: Vec<&'a str>, path: &'a str) -> Vec<&'a str> {
    if !paths.contains(&path) {
        paths.push(path);
    }
    paths
}

pub(super) fn ranked(probes: &[ReflectionProbe], camera_pos: Vec3) -> Vec<usize> {
    let mut order: Vec<usize> = (0..probes.len())
        .filter(|&i| !probes[i].cubemap_path.is_empty())
        .collect();
    let distance = |i: usize| probes[i].volume_distance_sq(camera_pos);
    order.sort_by(|&a, &b| {
        (!probes[a].fallback)
            .cmp(&!probes[b].fallback)
            .then(distance(a).total_cmp(&distance(b)))
            .then(a.cmp(&b))
    });
    order
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;

#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;

#[cfg(test)]
#[path = "format_tests.rs"]
mod format_tests;

#[cfg(test)]
#[path = "refuse_tests.rs"]
mod refuse_tests;

#[cfg(test)]
#[path = "fallback_tests.rs"]
mod fallback_tests;

#[cfg(test)]
#[path = "pick_tests.rs"]
mod pick_tests;
