//! The frame's shadow sweep (#435): pick the sun, fit the cascades to the base camera,
//! upload both uniforms, plan the point/spot shadow atlas (#468) and record the depth
//! passes — once per frame, before the camera stack draws, so every stacked camera
//! samples the same maps. A frame with no live sun fits no cascade, draws none and
//! uploads a count of 0, which the forward shader's `calculate_shadow` skips on
//! (#1713): a `Baked` sun costs no shadow work, as in Unity.

use glam::Vec3;

use super::cascades::Cascade;
use super::casters::CasterFrame;
use super::{atlas, cascades, CascadeUniform, ShadowRenderer};
use crate::components::{LightType, ShadowSettings};
use crate::render::clusters::{self, ClusterGrid};
use crate::render::lod::LodSelection;
use crate::render::Renderer;
use crate::scene::{Camera, Scene};

impl Renderer {
    /// Fit the cascades to `camera` (at `aspect`), plan the atlas for the lights it
    /// sees, upload the frame's local lights with their tiles, and render the shadow
    /// maps — the dynamic casters at the LOD levels `lod` (the base camera's) shows
    /// (#472). `settings` are the screen's base camera's volume's (#1434), the
    /// defaults without one.
    pub(crate) fn run_shadow_passes(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        aspect: f32,
        settings: &ShadowSettings,
        lod: &LodSelection,
    ) {
        let fitted = frame_cascades(scene, self.capture.static_only, camera, aspect, settings);
        // A probe captured without shadows (#1584) samples no cascade.
        let sampled: &[_] = if self.capture.shadows { &fitted } else { &[] };
        let uniform = CascadeUniform::new(sampled, camera.position, camera.forward());
        self.queue
            .write_buffer(&self.shadow_uniform_buffer, 0, bytemuck::bytes_of(&uniform));
        self.shadow_renderer.instancing = self.instancing;
        self.shadow_renderer.update_cascades(&self.queue, fitted);
        self.shadow_renderer
            .set_time(&self.queue, scene.shader_time);
        self.plan_shadow_atlas(scene, camera, aspect);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Shadow Encoder"),
            });
        if self.retained.scene(scene.id()).is_none() {
            self.extract_scene(scene); // swept with no frame prepared (a test)
        }
        // Measured from the base camera, never the light (#1714); a capture culls
        // nothing.
        let small = self.small_object_cull(camera);
        let objects = self.retained.scene(scene.id()).expect("extracted above");
        let frame = CasterFrame {
            device: &self.device,
            queue: &self.queue,
            scene,
            objects,
            gpu_meshes: &self.gpu_meshes,
            meshes: &self.retained.meshes,
            materials: &self.materials,
            surfaces: &self.surface_shaders,
            timer: &self.gpu_timer,
            statics: objects.statics_key(scene),
            small,
        };
        if self.capture.shadows {
            // a probe captured without shadows samples no cascade, so draws none;
            // without a live sun there is no cascade to draw (#1713)
            self.shadow_renderer
                .render_cascades(&mut encoder, &frame, lod);
        }
        let bake = self.shadow_renderer.render_atlas(&mut encoder, &frame, lod);
        self.queue.submit(std::iter::once(encoder.finish()));
        self.frame_counters.shadow_atlas_cached = bake.cached;
        self.frame_counters.shadow_atlas_rebaked = bake.rebaked;
    }
}

impl Renderer {
    /// Place the staged local lights' shadows in the atlas as `camera` (at `aspect`)
    /// sees them, upload the lights with their tiles, and count what fit (#468).
    /// Only lights the camera's light budget keeps are planned (#873). Each light
    /// keeps its tiles from frame to frame (#1711); a bake capture plans on its own,
    /// so it never moves them.
    fn plan_shadow_atlas(&mut self, scene: &Scene, camera: &Camera, aspect: f32) {
        let view_proj = camera.build_view_projection(aspect);
        let grid = ClusterGrid::new(camera, aspect);
        let mut requests = clusters::shadow_requests(&grid, &self.clusters.staged());
        if !self.capture.shadows {
            requests.clear(); // a probe captured without shadows plans no tile
        }
        let plan = if self.capture.static_only {
            atlas::plan(&requests, view_proj, camera.position)
        } else {
            let ids = local_light_ids(scene, false);
            debug_assert_eq!(ids.len(), requests.len(), "an id per staged light");
            let frame = atlas::Frame {
                lights: &requests,
                ids: &ids,
                view_proj,
                eye: camera.position,
            };
            let slots = &mut self.shadow_renderer.atlas.slots;
            slots.plan(Some(scene.id()), &frame, |id| scene.world.contains(id))
        };
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

/// The entity ids of the lights `clusters::local_lights(scene, capture)` stages, in
/// its order: the key each light's atlas tiles are kept under (#1711).
/// `frame_tests` holds the two in step.
pub(super) fn local_light_ids(scene: &Scene, capture: bool) -> Vec<u32> {
    let world = &scene.world;
    (world.ids_with_light().into_iter())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter(|&id| {
            world.light(id).is_some_and(|light| {
                light.effective_mode().renders_live(capture)
                    && matches!(
                        light.light_type,
                        LightType::Point | LightType::Spotlight | LightType::Area
                    )
            })
        })
        .collect()
}

/// The cascades the frame fits to `camera` (at `aspect`): `settings`' count for the
/// live sun, none without one (#1713), so nothing is swept, re-baked or sampled for a
/// `Baked` sun.
fn frame_cascades(
    scene: &Scene,
    capture: bool,
    camera: &Camera,
    aspect: f32,
    settings: &ShadowSettings,
) -> Vec<Cascade> {
    live_sun(scene, capture).map_or_else(Vec::new, |dir| {
        cascades::fit(camera, aspect, dir, settings, ShadowRenderer::CASCADE_SIZE)
    })
}

/// The direction the scene's (last) active directional light shines, or `None` when
/// there is none. Only lights the frame (or a bake `capture`) draws live count
/// (#809), so the cascades follow the light in the uniform's slot 0. The sun always
/// casts: its `cast_shadows` is not read.
fn live_sun(scene: &Scene, capture: bool) -> Option<Vec3> {
    let mut dir = None;
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        if light.light_type == LightType::Directional && light.mode.renders_live(capture) {
            dir = Some(scene.world.transform_direction(id, Vec3::NEG_Z).normalize());
        }
    }
    dir
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
