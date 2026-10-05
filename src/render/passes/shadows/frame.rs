//! The frame's shadow sweep (#435): pick the sun, fit the cascades to the base camera,
//! upload both uniforms, plan the point/spot shadow atlas (#468) and record the depth
//! passes — once per frame, before the camera stack draws, so every stacked camera
//! samples the same maps.

use glam::Vec3;

use super::casters::CasterFrame;
use super::{atlas, cascades, CascadeUniform, ShadowRenderer};
use crate::components::{LightType, ShadowSettings};
use crate::render::lod::LodSelection;
use crate::render::Renderer;
use crate::scene::{Camera, Scene};

impl Renderer {
    /// Fit the cascades to `camera` (at `aspect`), plan the atlas for the lights it
    /// sees, upload the frame's local lights with their tiles, and render the shadow
    /// maps — the dynamic casters at the LOD levels `lod` (the base camera's) shows
    /// (#472).
    pub(crate) fn run_shadow_passes(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        aspect: f32,
        lod: &LodSelection,
    ) {
        let fitted = cascades::fit(
            camera,
            aspect,
            sun_direction(scene, self.static_capture),
            &shadow_settings(scene),
            ShadowRenderer::CASCADE_SIZE,
        );
        let uniform = CascadeUniform::new(&fitted, camera.position, camera.forward());
        self.queue
            .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        self.shadow_renderer.instancing = self.instancing;
        self.shadow_renderer.update_cascades(&self.queue, fitted);
        self.shadow_renderer
            .set_time(&self.queue, scene.shader_time);
        self.plan_shadow_atlas(camera, aspect);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Encoder"),
            });
        let clips = self.shadow_clips(scene);
        let frame = CasterFrame {
            device: &self.device,
            queue: &self.queue,
            scene,
            gpu_meshes: &self.gpu_meshes,
            clips: &clips,
            materials: &self.materials,
            surfaces: &self.surface_shaders,
            timer: &self.gpu_timer,
        };
        self.shadow_renderer
            .render_cascades(&mut encoder, &frame, lod);
        let bake = self.shadow_renderer.render_atlas(&mut encoder, &frame, lod);
        self.queue.submit(std::iter::once(encoder.finish()));
        self.frame_counters.shadow_atlas_cached = bake.cached;
        self.frame_counters.shadow_atlas_rebaked = bake.rebaked;
    }
}

impl Renderer {
    /// Place the staged local lights' shadows in the atlas as `camera` (at `aspect`)
    /// sees them, upload the lights with their tiles, and count what fit (#468).
    fn plan_shadow_atlas(&mut self, camera: &Camera, aspect: f32) {
        let view_proj = camera.build_view_projection(aspect);
        let plan = atlas::plan(&self.clusters.staged(), view_proj, camera.position);
        for &(light, first_tile) in &plan.shadows {
            self.clusters.set_shadow(light, first_tile);
        }
        self.upload_local_lights();
        let c = &mut self.frame_counters;
        c.shadowed_lights = plan.shadows.len() as u32;
        c.shadow_lights_dropped = plan.dropped;
        c.shadow_atlas_tiles = plan.tiles.len() as u32;
        c.shadow_atlas_texels = plan.texels();
        self.shadow_renderer.atlas.update(&self.queue, plan.tiles);
    }
}

/// The direction the scene's (last) active directional light shines, or the default
/// sun's when there is none. Only lights the frame (or a bake `capture`) draws live
/// count (#809), so the cascades follow the light in the uniform's slot 0.
fn sun_direction(scene: &Scene, capture: bool) -> Vec3 {
    let mut dir = Vec3::new(-0.5, -1.0, -0.3).normalize();
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        if light.light_type == LightType::Directional && light.mode.renders_live(capture) {
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
