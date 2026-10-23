//! Choosing each frame's resident reflection probes (#1206): the baked probes nearest
//! the primary camera, their cubes loaded through the renderer's by-path cache.

use glam::Vec3;

use super::MAX_REFLECTION_PROBES;
use crate::render::Renderer;
use crate::scene::Scene;

impl Renderer {
    /// Make the baked probes nearest `camera_pos` this frame's resident probes
    /// (#1206), loading their cubes through the by-path cache (#355), and mark group 0
    /// for a rebind when the array texture was replaced. Ties in distance go to the
    /// lower probe index, so the set is the same every run.
    pub(crate) fn update_reflection_probes(&mut self, scene: &Scene, camera_pos: Vec3) {
        let probes = &scene.reflection_probes.probes;
        let mut order: Vec<usize> = (0..probes.len())
            .filter(|&i| !probes[i].cubemap_path.is_empty())
            .collect();
        let distance = |i: usize| probes[i].position.distance_squared(camera_pos);
        order.sort_by(|&a, &b| distance(a).total_cmp(&distance(b)).then(a.cmp(&b)));
        let mut wanted = Vec::new();
        for i in order {
            if wanted.len() == MAX_REFLECTION_PROBES {
                break;
            }
            if let Some(cube) = self.cached_cubemap(&probes[i].cubemap_path) {
                wanted.push((&probes[i], cube));
            }
        }
        if self
            .reflection_probes
            .update(&self.device, &self.queue, &wanted)
        {
            self.global_bind_group_dirty = true;
        }
    }
}
