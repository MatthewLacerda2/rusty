use crate::gpu::commands::Commands;
use crate::gpu::ops::CountedQueue as _;
use glam::Vec3;

use super::cascades::Cascade;
use super::casters::CasterFrame;
use super::{atlas, cascades, CascadeUniform, ShadowRenderer};
use crate::clusters::{self, ClusterGrid};
use crate::lod::LodSelection;
use crate::Renderer;
use rusty_components::{LightType, ShadowSettings};
use rusty_scene::{Camera, Scene};

impl Renderer {
    /// Once a frame, before the camera stack draws, so every stacked camera samples the same maps.
    pub(crate) fn run_shadow_passes(
        &mut self,
        commands: &mut Commands,
        scene: &Scene,
        (camera, aspect): (&Camera, f32),
        (settings, lod): (&ShadowSettings, &LodSelection),
    ) {
        let fitted = frame_cascades(scene, self.capture.static_only, camera, aspect, settings);
        let sampled: &[_] = if self.casts_shadows() { &fitted } else { &[] };
        let uniform = CascadeUniform::new(sampled, camera.position, camera.forward());
        self.queue.counted_write_buffer(
            &self.shadow_uniform_buffer,
            0,
            bytemuck::bytes_of(&uniform),
        );
        self.shadow_renderer.instancing = self.instancing;
        let masking = self.masking_statics(scene);
        let sun = live_sun_id(scene, false);
        self.shadow_renderer.sun_statics = !(masking && sun.is_some_and(|id| masked(scene, id)));
        self.shadow_renderer.update_cascades(&self.queue, fitted);
        self.shadow_renderer
            .set_time(&self.queue, self.capture.shader_time(scene));
        self.plan_shadow_atlas(scene, camera, aspect, masking);

        let encoder = commands.encoder();
        let small = self.small_object_cull(camera);
        let objects = self
            .retained
            .scene(scene.id())
            .expect("prepare_frame extracts");
        let frame = CasterFrame {
            device: &self.device,
            queue: &self.queue,
            scene,
            objects,
            gpu_meshes: &self.gpu_meshes,
            meshes: &self.retained.meshes,
            materials: &self.materials,
            records: &self.retained.records,
            surfaces: &self.surface_shaders,
            timer: &self.gpu_timer,
            statics: objects.statics_key(scene),
            small,
        };
        if self.casts_shadows() {
            self.shadow_renderer.render_cascades(encoder, &frame, lod);
        }
        let bake = self.shadow_renderer.render_atlas(encoder, &frame, lod);
        self.frame_counters.shadow_atlas_cached = bake.cached;
        self.frame_counters.shadow_atlas_rebaked = bake.rebaked;
    }
}

impl Renderer {
    fn plan_shadow_atlas(&mut self, scene: &Scene, camera: &Camera, aspect: f32, masking: bool) {
        let view_proj = camera.build_view_projection(aspect);
        let grid = ClusterGrid::new(camera, aspect);
        let mut requests = clusters::shadow_requests(&grid, &self.clusters.staged());
        let casts = self.casts_shadows();
        if !casts {
            requests.clear();
        }
        let plan = if self.capture.static_only || !casts {
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
        let mut plan = plan;
        for &(light, first_tile) in &plan.shadows {
            self.clusters.set_shadow(light, first_tile);
        }
        if masking {
            keep_masked_statics_out(scene, &requests, &mut plan);
        }
        self.upload_local_lights();
        let c = &mut self.frame_counters;
        c.shadowed_lights = plan.shadows.len() as u32;
        c.shadow_lights_dropped = plan.dropped;
        c.shadow_atlas_tiles = plan.tiles.len() as u32;
        c.shadow_atlas_texels = plan.texels();
        // A held-back resize is history: drawing the frame again counts towards it.
        c.history_passes += u32::from(plan.resizing);
        self.shadow_renderer.atlas.update(&self.queue, plan.tiles);
    }
}

impl Renderer {
    fn casts_shadows(&self) -> bool {
        self.switches.casts_shadows(self.capture.shadows)
    }

    fn masking_statics(&self, scene: &Scene) -> bool {
        use rusty_core::settings::quality::ShadowmaskMode;
        use rusty_scene::lighting::MixedLightingMode;
        let on = scene.lighting_settings.mixed_lighting_mode == MixedLightingMode::Shadowmask;
        let plain = self.quality.shadowmask_mode == ShadowmaskMode::Shadowmask;
        !self.capture.static_only && on && plain && self.lightmaps.masked() && !fog_on(scene)
    }
}

fn masked(scene: &Scene, id: u32) -> bool {
    scene.mask_channel(id).is_some()
}

pub(super) fn fog_on(scene: &Scene) -> bool {
    let world = &scene.world;
    world.ids_with_camera().into_iter().any(|id| {
        rusty_scene::camera::camera_volume(scene, id).is_some_and(|v| v.volumetric_fog.active)
    })
}

fn keep_masked_statics_out(
    scene: &Scene,
    requests: &[(clusters::LocalLight, bool)],
    plan: &mut atlas::AtlasPlan,
) {
    let ids = local_light_ids(scene, false);
    for &(light, first) in &plan.shadows {
        if !ids.get(light).is_some_and(|&id| masked(scene, id)) {
            continue;
        }
        let faces = if requests[light].0.one_frustum() {
            1
        } else {
            6
        };
        let first = first as usize;
        for tile in plan.tiles.iter_mut().skip(first).take(faces) {
            tile.statics = false;
        }
    }
}

/// In `clusters::local_lights`' order: each light's tiles are kept under its id.
pub(super) fn local_light_ids(scene: &Scene, capture: bool) -> Vec<u32> {
    let world = &scene.world;
    (world.ids_with_light().into_iter())
        .filter(|&id| world.is_active_in_hierarchy(id))
        .filter(|&id| {
            world.light(id).is_some_and(|light| {
                crate::clusters::drawn_mode(scene, id, &light).renders_live(capture)
                    && matches!(
                        light.light_type,
                        LightType::Point | LightType::Spotlight | LightType::Area
                    )
            })
        })
        .collect()
}

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

fn live_sun(scene: &Scene, capture: bool) -> Option<Vec3> {
    let id = live_sun_id(scene, capture)?;
    Some(scene.world.transform_direction(id, Vec3::NEG_Z).normalize())
}

fn live_sun_id(scene: &Scene, capture: bool) -> Option<u32> {
    let mut sun = None;
    for id in scene.world.ids_with_light() {
        if !scene.world.is_active_in_hierarchy(id) {
            continue;
        }
        let light = scene.world.light(id).expect("id came from ids_with_light");
        let live = crate::clusters::drawn_mode(scene, id, &light).renders_live(capture);
        if light.light_type == LightType::Directional && live {
            sun = Some(id);
        }
    }
    sun
}

#[cfg(test)]
#[path = "frame_tests.rs"]
mod frame_tests;
