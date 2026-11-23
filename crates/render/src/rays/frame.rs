//! A frame's hold on the tree: a top-only upload rewrites the nodes a recorded bind group
//! walks, so the frame takes it once before any pass records and keeps it until it ends.

use super::gpu;
use crate::Renderer;
use rusty_scene::Scene;

#[cfg(test)]
pub(crate) fn limit_for_tests(bytes: Option<u64>) {
    gpu::LIMIT.set(bytes);
}

impl Renderer {
    /// Held as taken until released; `None` when refused or not yet extracted.
    pub(crate) fn take_rays(&mut self, scene: &Scene) -> Option<wgpu::BindGroup> {
        self.refresh_rays(scene.id(), scene)?;
        let tree = &mut self.retained.scene_mut(scene.id())?.rays;
        tree.held = true;
        if tree.refused() {
            if !tree.fallback_said {
                log::warn!(
                    "[Render] SSR's Mixed tracing falls back to RayMarching in scene {:?}: its \
                     ray-traced tree does not fit this GPU",
                    scene.id()
                );
                tree.fallback_said = true;
            }
            return None;
        }
        tree.fallback_said = false;
        tree.bind_group().cloned()
    }

    pub(crate) fn release_rays(&mut self) {
        for rs in self.retained.scenes_mut() {
            rs.rays.held = false;
        }
    }
}
