//! The frame's shadow sweep (#435): pick the sun, fit the cascades to the base camera,
//! upload both uniforms and record the depth passes — once per frame, before the
//! camera stack draws, so every stacked camera samples the same maps.

use glam::Vec3;

use super::{cascades, CascadeUniform, ShadowRenderer};
use crate::components::{LightType, ShadowSettings};
use crate::render::Renderer;
use crate::scene::{Camera, Scene};

impl Renderer {
    /// Fit the cascades to `camera` (at `aspect`) and render the shadow maps.
    pub(crate) fn run_shadow_passes(&mut self, scene: &Scene, camera: &Camera, aspect: f32) {
        let fitted = cascades::fit(
            camera,
            aspect,
            sun_direction(scene),
            &shadow_settings(scene),
            ShadowRenderer::CASCADE_SIZE,
        );
        let uniform = CascadeUniform::new(&fitted, camera.position, camera.forward());
        self.queue
            .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        self.shadow_renderer.instancing = self.instancing;
        self.shadow_renderer.update_cascades(&self.queue, fitted);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Encoder"),
            });
        self.shadow_renderer.render(
            &self.device,
            &self.queue,
            &mut encoder,
            scene,
            &self.gpu_meshes,
        );
        self.queue.submit(std::iter::once(encoder.finish()));
    }
}

/// The direction the scene's (last) active directional light shines, or the default
/// sun's when there is none.
fn sun_direction(scene: &Scene) -> Vec3 {
    let mut dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        if light.light_type == LightType::Directional {
            let transform = scene.world.transform(id).expect("mandatory Transform");
            dir = (transform.rotation * Vec3::NEG_Z).normalize();
        }
    }
    dir
}

/// The first active visual-correction volume's shadow settings — the one
/// `Graphics.*Shadow*` writes — or the engine defaults without one.
fn shadow_settings(scene: &Scene) -> ShadowSettings {
    scene
        .world
        .ids_with_visual_correction()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .find_map(|id| {
            let vc = scene.world.visual_correction(id)?;
            vc.active.then_some(vc.shadows)
        })
        .unwrap_or_default()
}
