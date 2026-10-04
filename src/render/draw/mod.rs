//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

mod axis;
pub(crate) mod batch;
mod camera;
mod lighting;
pub(crate) mod materials;
mod overlays;
mod pass;
mod probes;
pub(crate) mod resources;
pub(crate) mod sort;
pub(crate) mod stack;
pub(crate) mod uniforms;

use glam::Vec3;

use self::stack::StackPass;
use crate::render::lod::LodSelection;
use crate::render::{build_camera_stack, texture_cameras, RenderView, Renderer};
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
        // The scene's baked lightmap pages (#438): rebinding group 0 only on a change.
        if self
            .lightmaps
            .bind(&self.device, &self.queue, &scene.lightmaps.pages)
        {
            self.global_bind_group_dirty = true;
        }

        // Build + write the camera-independent lighting uniform once. The reflection
        // probe is picked relative to the primary camera (#244).
        self.upload_lighting(scene, camera.position);
        // Fill the world-matrix store once for the whole frame (#331): every pass and
        // camera reads it instead of walking parent chains; rendering mutates nothing.
        scene.refresh_world_matrices();

        // Every decal and its maps, once for the whole frame (#638); each camera
        // bins them into its clusters. After the store: owned decals read it (#639).
        self.upload_decals(scene);
    }

    /// Renders the 3D scene into `view` (a per-view target/depth/post-FX bundle, #355),
    /// compositing the final image to `output`.
    ///
    /// In play mode this first draws the scene's render-texture cameras (#430) into
    /// their textures, then composites the screen camera stack (#93): the active
    /// `CameraComponent` entities, sorted by `render_order`, each with their own
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
        let textures = match editor_mode {
            true => Vec::new(),
            false => texture_cameras(camera, scene),
        };
        self.sync_render_textures(view, &textures);
        self.prepare_frame(view, scene, camera);

        // The ordered camera stack (one entry in edit mode / when no scene camera).
        let stack = build_camera_stack(camera, scene, !editor_mode);
        // The sun's cascades follow the base camera; every stacked camera — and every
        // render-texture camera (#430) — samples them. So do the dynamic casters' LOD
        // levels (#472): a shadow shows the level its caster shows in the main view.
        let base_lod = LodSelection::for_camera(scene, &stack[0]);
        self.run_shadow_passes(scene, &stack[0], view.aspect(), &base_lod);
        self.draw_render_textures(view, scene, &textures);
        // Lay out and mesh the UI through the base camera (#418, #429): world
        // canvases draw inside each camera's pass below, screen canvases after it.
        self.prepare_ui(view, scene, &stack[0], editor_mode);
        let pass = StackPass {
            stack: &stack,
            output,
            editor_mode,
            full_post_fx: true,
        };
        self.draw_stack(view, scene, pass);

        // The in-game UI over the finished frame (#418): after post-FX, so a HUD
        // is never tonemapped, bloomed or FXAA-softened. Not in the Scene view
        // unless its UI overlay is on (#423).
        if !editor_mode || view.ui.screen_in_editor {
            self.draw_ui(view);
        }
        self.finish_counters(view);
    }

    /// Upload/refresh per-frame GPU assets (meshes, textures, skybox) shared by every
    /// camera in the stack, and mark re-baked surface shaders for a rebuild (#396).
    fn upload_scene_assets(&mut self, scene: &Scene) {
        // Re-baked or newly present texture files drop their stale uploads (#689).
        self.refresh_textures();
        self.upload_scene_meshes(scene);
        self.surface_shaders.refresh();
        // An entity whose last shader-param override was cleared draws with its
        // material's shared group again; free the buffer and group it owned (#670).
        let overrides = &scene.shader_overrides;
        self.materials
            .release_entities(scene.id(), |id| overrides.has(id));
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
/// normal, emissive, then the extra shader texture slots, #400) the forward shader samples — gathered as owned strings so the
/// scene borrow ends before the textures are uploaded (#202, #207).
pub(crate) fn active_material_map_paths(scene: &Scene) -> Vec<String> {
    scene
        .world
        .ids_with_material()
        .into_iter()
        .filter(|&id| scene.world.is_active(id))
        .filter_map(|id| scene.material_asset_of(id).cloned())
        .flat_map(|m| materials::material_texture_paths(Some(&m)))
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "instancing_tests.rs"]
mod instancing_tests;
