//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

mod lighting;
mod overlays;
mod pass;
mod path;
mod probes;
pub(crate) mod resources;
mod uniforms;

use glam::Vec3;

use self::lighting::{
    apply_reflection_probe, apply_scene_lights, apply_ssr_settings, default_lighting_uniform,
};
use self::pass::{PassClear, ScenePassFrame};
use crate::render::postfx::params::build_post_params;
use crate::render::{
    build_camera_stack, Camera, CameraUniform, LightingUniform, RenderView, Renderer,
};
use crate::scene::Scene;

impl Renderer {
    /// Renders the 3D scene into `view` (a per-view target/depth/post-FX bundle, #355),
    /// compositing the final image to `output`.
    ///
    /// In play mode this composites the camera stack (#93): the scene's active
    /// `CameraComponent` entities, sorted by `render_order`, each draw with their own
    /// culling mask / lens / clear flags so a viewmodel or UI camera layers on top of
    /// the world. In edit mode the free-fly `camera` is the single pass. Post-FX runs
    /// once over the composited HDR target.
    // A `(view, scene, camera, output, mode, paths)` render entry: each argument is a
    // distinct, irreducible input to one frame, so the width is inherent (#355).
    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
        editor_mode: bool,
        pathfinding_points: &[Vec3],
    ) {
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
        let lighting_uniform = self.build_lighting_uniform(scene, camera.position);
        self.queue.write_buffer(
            &self.lighting_buffer,
            0,
            bytemuck::bytes_of(&lighting_uniform),
        );

        // Drop pool slots for entities no longer active, so the persistent forward
        // buffers track the live scene rather than growing without bound (#210).
        self.prune_entity_pool(scene);

        // Fill the world-matrix store once for the whole frame (#331). The solid pass and
        // both shadow collects — across every camera in the stack — then read each entity's
        // world matrix from this one O(N) fill instead of walking the parent chain per
        // entity per consumer. Nothing mutates transforms during rendering, so one refresh
        // here serves the entire frame.
        scene.refresh_world_matrices();
        let aspect = view.aspect();

        // The ordered camera stack (one entry in edit mode / when no scene camera).
        let stack = build_camera_stack(camera, scene, !editor_mode);
        let last = stack.len().saturating_sub(1);
        // The base (first) camera drives the shared post-FX history / motion vectors.
        let base_view_proj = stack[0].build_view_projection(aspect);

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
            };
            self.queue
                .write_buffer(&self.camera_buffer, 0, bytemuck::bytes_of(&camera_uniform));

            // 2. Sync per-camera resources (the culling mask differs per camera). The
            // forward entity buffers/bind groups persist in the pool; this only
            // rewrites their contents and returns lightweight draw items (#210). The
            // split separates opaque/cutout (the solids pass) from transparent (the
            // sorted alpha-blended pass below) (#242).
            let solids = self.precreate_solid_resources(scene, cam, &frustum);
            let overlays = self.precreate_overlays(scene, editor_mode);
            let _path_resources = self.precreate_path(pathfinding_points);

            let frame = ScenePassFrame {
                editor_mode,
                clear: PassClear::for_pass(idx == 0, cam.clear_flags),
            };
            self.execute_scene_pass(view, scene, frame, &solids.opaque, &overlays);

            // Project box-decals over this camera's lit surfaces (reads the scene
            // depth to reconstruct geometry), after solids/skybox and before the
            // additive particles so decals sit on the surface, not over the sparks.
            self.draw_decals(view, scene, cam);

            // Translucent solids (#242): alpha-blended, depth-tested against opaque,
            // drawn back-to-front (already sorted) after opaque + decals so glass
            // composites over the world behind it.
            self.draw_transparent(view, &solids.transparent);

            // Billboard particles for this camera (after solids, before the next pass).
            self.draw_particles(view, scene, cam);

            // 3. Composite + post-process once, over the final pass's HDR target.
            if idx == last {
                self.run_post_fx(view, scene, output, base_view_proj, cam.position.to_array());
            }
        }
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

    /// Evict `scene`'s persistent forward-pass slots for entities no longer active
    /// in it, keeping the pool bounded to the live set (#210). Scoped to this scene's
    /// own slots, so rendering a second scene never evicts the first's (#355).
    fn prune_entity_pool(&mut self, scene: &Scene) {
        let live: std::collections::HashSet<u32> = scene
            .entity_ids()
            .into_iter()
            .filter(|&id| scene.world.is_active(id))
            .collect();
        if let Some(pool) = self.entity_pool.as_mut() {
            pool.retain(scene.id(), &live);
        }
        self.shadow_renderer.retain_entities(scene.id(), &live);
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
