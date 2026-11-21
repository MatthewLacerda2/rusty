pub(crate) mod batch;
pub(crate) mod camera;
mod detail;
mod lighting;
pub(crate) mod map_slots;
pub(crate) use map_slots::material_map_paths;
pub(crate) mod materials;
mod overlays;
pub(crate) mod pass;
pub(crate) mod proxy_volume;
pub(crate) mod record;
pub(crate) mod resources;
mod solid_key;
pub(crate) mod stack;
pub(crate) mod uniforms;
pub(crate) mod visibility;

#[cfg(test)]
mod inactive_parent_tests;

#[cfg(test)]
pub(crate) use lighting::scene_ambient;
pub(crate) use overlays::{ball_pipeline, NavMeshGizmo};
pub use overlays::{NavMeshPatch, NavMeshRevision, NavMeshSource};

use self::stack::StackPass;
use crate::gpu::commands::Commands;
use crate::lod::LodSelection;
use crate::postfx::effects::taa;
use crate::postfx::params::borrow_post_source;
use crate::{build_camera_stack, texture_cameras, GpuTexture, RenderView, Renderer};
use rusty_scene::{Camera, Scene};

impl Renderer {
    fn prepare_frame(&mut self, view: &mut RenderView, scene: &Scene, camera: &Camera) {
        self.lap("setup");
        self.open_notes(scene);
        let size = view.size();
        view.resize(&self.device, size.width, size.height);

        self.upload_scene_assets(scene);
        self.lap("assets");
        if !self.capture.static_only {
            self.proxy_volumes.begin_frame(scene);
        }

        self.update_reflection_probes(scene, camera.position);
        let [pages, directions, masks] = self.switches.lightmap_lists(&scene.lightmaps);
        let gpu = (&self.queue, &self.staging);
        if (self.lightmaps).bind(&self.device, gpu, pages, directions, masks) {
            self.global_bind_group_dirty = true;
        }
        self.note_scene_lightmaps(scene);

        self.global_bind_group_dirty |= self.fog_volume.tick(&self.device);
        self.upload_lighting(scene);
        self.note_unbaked(scene);
        self.lap("prepare");
        self.extract_scene(scene);
        self.lap("extract");
        let gpu = (&self.device, &self.queue);
        let panorama = self.skybox_texture.as_deref();
        let settings = crate::gpu::uniforms::ProceduralSkyUniform::from_settings(&scene.sky);
        let source = (panorama, &settings);
        let sky = self.sky_reflection.prepare(gpu, &self.gpu_timer, source);
        self.frame_counters.sky_reflection_bakes += u32::from(sky.baked);
        self.global_bind_group_dirty |= sky.rebind;
        self.spare_global_group.switched |= sky.switched;
        self.upload_decals(scene);
        self.step_gpu_particles(scene);
        self.lap("prepare");
    }

    pub fn render(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
        editor_mode: bool,
    ) {
        self.begin_frame_clocks();
        let main = rusty_scene::camera::main_camera(scene).filter(|_| !editor_mode);
        let mut blended = camera.clone();
        if let Some(id) = main {
            self.draw_poses.pose_camera(&mut blended, scene, id);
        }
        let camera = &blended;
        let textures = match editor_mode {
            true => Vec::new(),
            false => texture_cameras(camera, scene, &self.draw_poses),
        };
        self.sync_render_textures(view, &textures);
        self.prepare_frame(view, scene, camera);

        let screen = build_camera_stack(camera, scene, !editor_mode, &self.draw_poses);
        let stack = screen.cameras;
        let guards = borrow_post_source(scene, screen.base_id);
        let post = guards.source();
        let shadows = post.volume.map(|vc| vc.shadows).unwrap_or_default();
        let base_lod = LodSelection::for_camera(scene, &stack[0]);
        self.lap("setup");
        let mut commands = Commands::new(&self.device, &self.queue, "Frame Encoder");
        let lights = (&shadows, &base_lod);
        self.run_shadow_passes(&mut commands, scene, (&stack[0], view.aspect()), lights);
        self.lap("shadows");
        let mut commands = self.draw_render_textures(view, scene, &textures, commands);
        self.lap("render_textures");
        self.prepare_ui(&mut commands, view, scene, &stack[0], editor_mode);
        self.lap("ui");
        let graphics = self.graphics(editor_mode);
        let taa_on = taa::runs(view, post, graphics, true, editor_mode);
        let scaled = view.scaled_output(&self.device, !taa::upscales(view, taa_on));
        let pass = StackPass {
            stack: &stack,
            entities: &screen.entities,
            base_id: screen.base_id,
            post,
            output: scaled.as_ref().unwrap_or(output),
            editor_mode,
            full_post_fx: true,
            motion_frames: 1,
        };
        let commands = self.draw_stack(view, scene, (pass, commands));
        self.finish_screen(commands, view, (scaled.as_ref(), output), editor_mode);
        self.finish_main_frame(view, scene);
        self.gpu_timer.end_frame(&self.device, &self.queue);
    }

    fn finish_screen(
        &mut self,
        mut commands: Commands,
        view: &mut RenderView,
        (scaled, output): (Option<&wgpu::TextureView>, &wgpu::TextureView),
        editor_mode: bool,
    ) {
        if let Some(scaled) = scaled {
            self.upscale_scene(commands.encoder(), view, scaled, output);
            self.lap("upscale");
        }
        if !editor_mode || view.ui.screen_in_editor {
            self.draw_ui(&mut commands, view);
            self.lap("ui");
        }
        commands.send();
        self.lap("submit");
    }

    pub(crate) fn render_capture_face(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
    ) {
        self.begin_frame_clocks();
        self.prepare_frame(view, scene, camera);
        let shadows = borrow_post_source(scene, rusty_scene::camera::main_camera(scene))
            .source()
            .volume
            .map(|vc| vc.shadows)
            .unwrap_or_default();
        let lod = LodSelection::for_camera(scene, camera);
        let mut commands = Commands::new(&self.device, &self.queue, "Capture Encoder");
        let at = (camera, view.aspect());
        self.run_shadow_passes(&mut commands, scene, at, (&shadows, &lod));
        let pass = StackPass {
            stack: std::slice::from_ref(camera),
            entities: &[None],
            base_id: None,
            post: Default::default(),
            output,
            editor_mode: false,
            full_post_fx: false,
            motion_frames: 1,
        };
        self.draw_stack(view, scene, (pass, commands)).send();
        self.finish_counters(view);
        self.gpu_timer.end_frame(&self.device, &self.queue);
    }

    fn upload_scene_assets(&mut self, scene: &Scene) {
        self.refresh_textures();
        self.grade_bake.luts.refresh();
        self.load_lens_dirt(scene);
        if self.surface_shaders.refresh() {
            self.retained.touch();
        }
        let overrides = &scene.shader_overrides;
        self.materials
            .release_entities(scene.id(), |id| overrides.has(id));

        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            self.note_map(scene.id(), &scene.skybox_path);
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_panorama(&path));
            self.global_bind_group_dirty = true;
        } else if scene.skybox_path.is_empty() && self.skybox_texture.is_some() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
            self.global_bind_group_dirty = true;
        } else if !scene.skybox_path.is_empty() && self.notes_due() {
            self.note_map(scene.id(), &scene.skybox_path);
        }
    }

    pub(crate) fn sky_panorama(&self) -> &GpuTexture {
        self.skybox_texture
            .as_deref()
            .unwrap_or(&self.default_texture)
    }
}
