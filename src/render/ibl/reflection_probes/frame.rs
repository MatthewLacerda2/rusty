//! Choosing each frame's resident reflection probes (#1206): the baked probes whose
//! boxes are nearest the primary camera (#1238), as many as the scene's byte budget
//! holds (#1584), their cubes loaded through the renderer's by-path cache.

use glam::Vec3;

use super::MAX_REFLECTION_PROBES;
use crate::render::Renderer;
use crate::scene::{ReflectionProbe, Scene};

impl Renderer {
    /// Make the baked probes whose boxes are nearest `camera_pos` this frame's
    /// resident probes (#1206, #1238), loading their cubes through the by-path cache
    /// (#355), and mark group 0 for a rebind when the array texture was replaced.
    /// They are taken nearest first until the next would put the array past the
    /// scene's budget (`LightingSettings::reflections`, #1584); the nearest is always
    /// taken, whatever its size.
    /// Cubes that don't fit the array are passed over before the budget is counted,
    /// and never uploaded (#1245). Which probes are resident depends on the camera's
    /// distance to each box; only ties are broken by scene index, and the residents
    /// are handed over in scene order, so two runs from one camera agree.
    pub(crate) fn update_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) {
        let probes = &scene.reflection_probes.probes;
        let budget = scene.lighting_settings.reflections.budget_bytes();
        let mut wanted = Vec::new();
        let (mut face, mut paths) = (0, Vec::new());
        for i in ranked(probes, camera_pos) {
            if wanted.len() == MAX_REFLECTION_PROBES {
                break;
            }
            let Some(cube) = self.cached_cubemap(&probes[i].cubemap_path) else {
                continue;
            };
            let path = probes[i].cubemap_path.as_str();
            let cubes = paths.len() + usize::from(!paths.contains(&path));
            let grown = face.max(cube.size);
            if !wanted.is_empty() && array_bytes(grown, cubes) > budget {
                break;
            }
            (face, paths) = (grown, push_new(paths, path));
            wanted.push((i, cube));
        }
        wanted.sort_by_key(|&(i, _)| i);
        let wanted: Vec<_> = wanted.into_iter().map(|(i, c)| (&probes[i], c)).collect();
        if self
            .reflection_probes
            .update(&self.device, &self.queue, &wanted)
        {
            self.global_bind_group_dirty = true;
        }
    }
}

/// The bytes an array of `cubes` cubes at a `face`-texel face takes: six RGBA8
/// faces each, and a third more for the mips.
pub(super) fn array_bytes(face: u32, cubes: usize) -> u64 {
    let level = u64::from(face) * u64::from(face) * 4 * 6;
    level * 4 / 3 * cubes as u64
}

/// `paths` with `path` added, if it is not there yet.
fn push_new<'a>(mut paths: Vec<&'a str>, path: &'a str) -> Vec<&'a str> {
    if !paths.contains(&path) {
        paths.push(path);
    }
    paths
}

/// The indices of the baked probes, nearest box to `camera_pos` first (zero inside
/// it), ties to the lower scene index, so the order is the same every run.
fn ranked(probes: &[ReflectionProbe], camera_pos: Vec3) -> Vec<usize> {
    let mut order: Vec<usize> = (0..probes.len())
        .filter(|&i| !probes[i].cubemap_path.is_empty())
        .collect();
    let distance = |i: usize| probes[i].box_distance_sq(camera_pos);
    order.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(a.cmp(&b)));
    order
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
