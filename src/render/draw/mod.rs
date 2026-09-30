//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

mod axis;
pub(crate) mod batch;
mod lighting;
mod materials;
mod overlays;
mod pass;
mod probes;
pub(crate) mod resources;
pub(crate) mod sort;
pub(crate) mod uniforms;

use glam::Vec3;

use self::lighting::{
    apply_reflection_probe, apply_scene_lights, apply_ssr_settings, default_lighting_uniform,
};
use self::pass::{PassClear, ScenePassFrame};
use crate::render::gpu::uniforms::FogUniform;
use crate::render::lod::LodSelection;
use crate::render::passes::ssao::{SsaoFrame, SsaoPlan};
use crate::render::postfx::params::build_post_params;
use crate::render::{build_camera_stack, CameraUniform, LightingUniform, RenderView, Renderer};
use crate::scene::{Camera, Scene};

impl Renderer {
    /// Per-frame work that doesn't depend on the camera stack: the view's post-FX
    /// buffers, scene assets, the reflection cube, the lighting uniform and the
    /// world-matrix store.
    fn prepare_frame(&mut self, view: &mut RenderView, scene: &Scene, camera: &Camera) {
        // Keep this view's post-FX bloom buffers sized to the active quality tier — a
        // cheap no-op unless a live quality switch changed the divisor (#355).
        let size = view.size();
        view.resize(
            &self.device,
            size.width,
            size.height,
            self.quality.bloom_divisor(),
        );

        self.upload_scene_assets(scene);

        // Load the active reflection probe's baked cubemap for the primary camera (#245),
        // so the forward pass reflects that prefiltered cube instead of the skybox.
        self.update_reflection_cube(scene, camera.position);

        // Build + write the camera-independent lighting uniform once. The reflection
        // probe is picked relative to the primary camera (#244).
        self.upload_lighting(scene, camera.position);

        // Fill the world-matrix store once for the whole frame (#331): every pass and
        // camera reads it instead of walking parent chains; rendering mutates nothing.
        scene.refresh_world_matrices();
    }

    /// Renders the 3D scene into `view` (a per-view target/depth/post-FX bundle, #355),
    /// compositing the final image to `output`.
    ///
    /// In play mode this composites the camera stack (#93): the scene's active
    /// `CameraComponent` entities, sorted by `render_order`, each draw with their own
    /// culling mask / lens / clear flags so a viewmodel or UI camera layers on top of
    /// the world. In edit mode the free-fly `camera` is the single pass. Post-FX runs
    /// once over the composited HDR target.
    pub fn render(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
        editor_mode: bool,
    ) {
        self.prepare_frame(view, scene, camera);
        let aspect = view.aspect();

        // The ordered camera stack (one entry in edit mode / when no scene camera).
        let stack = build_camera_stack(camera, scene, !editor_mode);
        // The sun's cascades follow the base camera; every stacked camera samples them.
        // So do the dynamic casters' LOD levels (#472): a shadow shows the level its
        // caster shows in the main view.
        let base_lod = LodSelection::for_camera(scene, &stack[0]);
        self.run_shadow_passes(scene, &stack[0], aspect, &base_lod);
        let last = stack.len().saturating_sub(1);
        // The base (first) camera drives the shared post-FX history / motion vectors.
        let base_view_proj = stack[0].build_view_projection(aspect);
        let ssao = SsaoPlan::for_scene(scene, self.quality);
        // Lay out and mesh the UI through the base camera (#418, #429): world
        // canvases draw inside each camera's pass below, screen canvases after it.
        self.prepare_ui(view, scene, &stack[0], editor_mode);

        for (idx, cam) in stack.iter().enumerate() {
            // 1. Write this camera's view/projection uniform.
            let view_proj = cam.build_view_projection(aspect);
            // This camera's world-space frustum, for culling off-screen entities (#330).
            // Built per camera because each stacked camera has its own lens/orientation;
            // the reflection-capture faces call `render` per face and so get it for free.
            let frustum = crate::render::Frustum::from_view_proj(view_proj);
            let camera_uniform = CameraUniform {
                view_proj: view_proj.to_cols_array(),
                camera_pos: cam.position.to_array(),
                _pad: 0.0,
                fog: FogUniform::from_settings(&scene.fog),
            };
            self.queue
                .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

            // 2. Batch this camera's solids (the culling mask differs per camera) into
            // instanced draws and upload their packed data (#470). The split separates
            // opaque/cutout (the solids pass) from transparent (the sorted
            // alpha-blended pass below) (#242).
            // Each camera picks its own LOD levels (#472), before batching so one level
            // of one prop is still one instanced draw.
            let lod = LodSelection::for_camera(scene, cam);
            let solids = self.precreate_solid_resources(scene, cam, &frustum, &lod);
            let overlays = self.precreate_overlays(scene, editor_mode);

            let frame = ScenePassFrame {
                editor_mode,
                clear: PassClear::for_pass(idx == 0, cam.clear_flags),
                ssao: ssao.map(|plan| SsaoFrame::for_camera(plan, view_proj, cam)),
            };
            self.execute_scene_pass(view, frame, &solids.draws.opaque, &overlays);

            // Project box-decals over this camera's lit surfaces (reads the scene
            // depth to reconstruct geometry), after solids/skybox and before the
            // additive particles so decals sit on the surface, not over the sparks.
            self.draw_decals(view, scene, cam);

            // Translucent solids (#242): alpha-blended, depth-tested against opaque,
            // drawn back-to-front (already sorted) after opaque + decals so glass
            // composites over the world behind it.
            self.draw_transparent(view, &solids.draws.transparent);
            // World canvases (#429): scene geometry, occluded by it, before particles.
            self.draw_world_ui(view, cam, aspect, FogUniform::from_settings(&scene.fog));

            // Sprite particles for this camera (after solids, before the next pass),
            // emitters back to front; mesh particles already drew with the solids.
            let particle_draws = self.draw_particles(view, scene, cam);
            self.count_camera(&solids, scene.decals.len(), particle_draws);

            // 3. Composite + post-process once, over the final pass's HDR target.
            if idx == last {
                self.run_post_fx(view, scene, output, base_view_proj, cam.position.to_array());
            }
        }

        // 4. The in-game UI over the finished frame (#418): after post-FX, so a HUD
        // is never tonemapped, bloomed or FXAA-softened. Not in the Scene view.
        if !editor_mode {
            self.draw_ui(view);
        }
        self.finish_counters(view);
    }

    /// Upload/refresh per-frame GPU assets (meshes, textures, skybox) shared by every
    /// camera in the stack. Borrow-splits cleanly before the render passes begin.
    fn upload_scene_assets(&mut self, scene: &Scene) {
        self.upload_scene_meshes(scene);

        // The material maps the shader samples (albedo #201, metallic/roughness #202),
        // collected paths-only first to end the scene borrow before uploading.
        for path in active_material_map_paths(scene) {
            self.load_texture(&path);
        }

        // Update skybox texture if path changed, marking the global bind group dirty
        // so it is rebuilt once (next frame) rather than every camera every frame —
        // the only thing in group 0 that changes outside its persistent buffers (#210).
        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_texture(&path));
            self.global_bind_group_dirty = true;
        } else if scene.skybox_path.is_empty() && self.skybox_texture.is_some() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
            self.global_bind_group_dirty = true;
        }
    }

    /// Run the post-process chain (color correction, bloom, motion blur, SSR) over the
    /// view's composited HDR target, writing the corrected image to `output`.
    fn run_post_fx(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        output: &wgpu::TextureView,
        view_proj: glam::Mat4,
        camera_pos: [f32; 3],
    ) {
        use crate::render::postfx::PostFxContext;

        let (mut post_params, bloom_enabled) = build_post_params(
            scene,
            self.quality,
            view_proj,
            view.post_fx.prev_view_proj,
            camera_pos,
        );
        post_params.misc[1] = 1.0 / view.post_fx.bloom_size.0 as f32;
        post_params.misc[2] = 1.0 / view.post_fx.bloom_size.1 as f32;
        view.post_fx.prev_view_proj = view_proj;

        let skybox_view = self
            .skybox_texture
            .as_ref()
            .map(|tex| &tex.view)
            .unwrap_or(&self.default_texture.view);
        let ctx = PostFxContext {
            depth_view: &view.depth_view,
            skybox_view,
            output,
        };
        let passes = crate::render::postfx::PostPasses {
            bloom: bloom_enabled,
            fxaa: crate::render::postfx::params::fxaa_enabled(scene),
        };
        view.post_fx
            .run(&self.device, &self.queue, ctx, post_params, passes);
    }

    /// Write the frame's lighting uniform, and start the frame counters (#433) with
    /// the light counts — which lights got a uniform slot is decided right here.
    fn upload_lighting(&mut self, scene: &Scene, camera_pos: Vec3) {
        let lighting_uniform = self.build_lighting_uniform(scene, camera_pos);
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );
        self.begin_counters(scene);
    }

    /// Builds the per-frame lighting uniform from the scene's lights and SSR settings.
    /// `refl_has_cubemap` is set only when a baked cube is actually loaded for the active
    /// probe (`self.reflection_cube`), so the shader never samples the black fallback cube.
    fn build_lighting_uniform(&self, scene: &Scene, camera_pos: Vec3) -> LightingUniform {
        let mut lighting_uniform = default_lighting_uniform(scene);
        apply_scene_lights(&mut lighting_uniform, scene);
        apply_ssr_settings(&mut lighting_uniform, scene);
        apply_reflection_probe(&mut lighting_uniform, scene, camera_pos);
        if self.reflection_cube.is_some() {
            lighting_uniform.refl_has_cubemap = 1.0;
        }
        lighting_uniform
    }

    /// Bind (or clear) the active reflection probe's baked cubemap for `camera_pos` (#245):
    /// the nearest probe whose box covers the camera and that carries a baked `cubemap_path`
    /// wins. Rebinding happens only when the active path changes (tracked by
    /// `reflection_cube_path`), and the group-0 bind group is marked dirty so the cube swaps
    /// in. A missing/unreadable file (or no probe) clears the cube and falls back to skybox.
    ///
    /// The cube itself comes from the by-path content cache, so two scenes flipping
    /// between different probes cost a hash lookup, not a disk read (#355).
    fn update_reflection_cube(&mut self, scene: &Scene, camera_pos: Vec3) {
        let path = scene
            .reflection_probes
            .select(camera_pos)
            .map(|p| p.cubemap_path.clone())
            .filter(|p| !p.is_empty())
            .unwrap_or_default();
        if path == self.reflection_cube_path {
            return;
        }
        self.reflection_cube_path = path.clone();
        self.reflection_cube = if path.is_empty() {
            None
        } else {
            self.cached_cubemap(&path)
        };
        self.global_bind_group_dirty = true;
    }
}

/// Every active entity's resolved material map paths (albedo, metallic, roughness,
/// normal, emissive) the forward shader samples — gathered as owned strings so the
/// scene borrow ends before the textures are uploaded (#202, #207).
fn active_material_map_paths(scene: &Scene) -> Vec<String> {
    scene
        .world
        .ids_with_material()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| scene.material_asset_of(id).cloned())
        .flat_map(|m| {
            [
                m.base_color_map,
                m.metallic_map,
                m.roughness_map,
                m.normal_map,
                m.emissive_map,
            ]
        })
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "instancing_tests.rs"]
mod instancing_tests;
