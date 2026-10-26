//! Top-level per-frame rendering: mesh/texture upload, uniform updates, and the
//! orchestration of pre-created resources into the scene render pass. Extracted
//! from the original monolithic `Renderer::render` (behavior unchanged).

pub(crate) mod batch;
mod camera;
mod lighting;
pub(crate) mod materials;
mod overlays;
mod pass;
pub(crate) mod resources;
pub(crate) mod stack;
pub(crate) mod uniforms;

#[cfg(test)]
mod inactive_parent_tests;

use self::stack::StackPass;
use crate::asset::MapRef;
use crate::render::gpu::textures::TextureKind;
use crate::render::lod::LodSelection;
use crate::render::postfx::params::borrow_post_source;
use crate::render::{build_camera_stack, texture_cameras, GpuTexture, RenderView, Renderer};
use crate::scene::{Camera, Scene};

impl Renderer {
    /// Per-frame work that doesn't depend on the camera stack: the view's post-FX
    /// buffers, scene assets, the resident reflection probes and the lighting uniform.
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

        // The baked reflection probes nearest the primary camera (#245, #1206); each
        // draw then reflects the one whose box holds it.
        self.update_reflection_probes(scene, camera.position);
        // The scene's baked lightmap pages (#438) and their direction pages (#810):
        // rebinding group 0 only on a change.
        let set = &scene.lightmaps;
        if self
            .lightmaps
            .bind(&self.device, &self.queue, &set.pages, &set.directions)
        {
            self.global_bind_group_dirty = true;
        }

        // Build + write the camera-independent lighting uniform once.
        self.upload_lighting(scene);
        // Every decal and its maps, once for the whole frame (#638); each camera
        // bins them into its clusters.
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
        self.gpu_timer.begin_frame();
        self.sync_render_textures(view, &textures);
        self.prepare_frame(view, scene, camera);

        // The ordered camera stack (one entry in edit mode / when no scene camera).
        let screen = build_camera_stack(camera, scene, !editor_mode);
        let stack = screen.cameras;
        // The screen's base camera's components, borrowed once for its post-FX,
        // SSAO and the sun's shadows (#1434).
        let guards = borrow_post_source(scene, screen.base_id);
        let post = guards.source();
        let shadows = post.volume.map(|vc| vc.shadows).unwrap_or_default();
        // The sun's cascades follow the base camera; every stacked camera — and every
        // render-texture camera (#430) — samples them. So do the dynamic casters' LOD
        // levels (#472): a shadow shows the level its caster shows in the main view.
        let base_lod = LodSelection::for_camera(scene, &stack[0]);
        self.run_shadow_passes(scene, &stack[0], view.aspect(), &shadows, &base_lod);
        self.draw_render_textures(view, scene, &textures);
        // Lay out and mesh the UI through the base camera (#418, #429): world
        // canvases draw inside each camera's pass below, screen canvases after it.
        self.prepare_ui(view, scene, &stack[0], editor_mode);
        let pass = StackPass {
            stack: &stack,
            entities: &screen.entities,
            base_id: screen.base_id,
            post,
            output,
            editor_mode,
            full_post_fx: true,
            motion_frames: 1,
        };
        self.draw_stack(view, scene, pass);

        // The in-game UI over the finished frame (#418): after post-FX, so a HUD
        // is never tonemapped, bloomed or FXAA-softened. Not in the Scene view
        // unless its UI overlay is on (#423).
        if !editor_mode || view.ui.screen_in_editor {
            self.draw_ui(view);
        }
        self.finish_counters(view);
        self.gpu_timer.end_frame(&self.device, &self.queue);
    }

    /// Upload/refresh per-frame GPU assets (meshes, textures, skybox) shared by every
    /// camera in the stack, and mark re-baked surface shaders for a rebuild (#396).
    fn upload_scene_assets(&mut self, scene: &Scene) {
        // Re-baked or newly present texture files drop their stale uploads (#689).
        self.refresh_textures();
        // And the Color Lookups the views' grading bakes read (#1441).
        self.grade_bake.luts.refresh();
        self.upload_scene_meshes(scene);
        self.surface_shaders.refresh();
        // An entity whose last shader-param override was cleared draws with its
        // material's shared group again; free the buffer and group it owned (#670).
        let overrides = &scene.shader_overrides;
        self.materials
            .release_entities(scene.id(), |id| overrides.has(id));
        // The material maps the shader samples (albedo #201, metallic/roughness #202),
        // collected paths-only first to end the scene borrow before uploading.
        // Every entity's, active or not: re-enabling one must not stall on uploads
        // and mip generation (#1363).
        for (path, kind) in material_map_paths(scene, |_| true) {
            self.load_texture_as(&path, kind);
        }

        // Update skybox texture if path changed, marking the global bind group dirty
        // so it is rebuilt once (next frame) rather than every camera every frame —
        // the only thing in group 0 that changes outside its persistent buffers (#210).
        if !scene.skybox_path.is_empty() && self.skybox_path != scene.skybox_path {
            let path = scene.skybox_path.clone();
            self.skybox_path = path.clone();
            self.skybox_texture = Some(self.load_panorama(&path));
            self.global_bind_group_dirty = true;
        } else if scene.skybox_path.is_empty() && self.skybox_texture.is_some() {
            self.skybox_path = "".to_string();
            self.skybox_texture = None;
            self.global_bind_group_dirty = true;
        }
    }

    /// The skybox panorama every pass that reads the sky binds (forward reflections,
    /// post-FX's SSR fallback), or the default texture when the scene has none.
    pub(crate) fn sky_panorama(&self) -> &GpuTexture {
        self.skybox_texture
            .as_deref()
            .unwrap_or(&self.default_texture)
    }
}

/// The resolved material map paths (albedo, metallic, roughness, normal, emissive,
/// then the extra shader texture slots, #400) the forward shader samples, of every
/// entity `admit` passes, gathered as owned `MapRef`s so the scene borrow ends
/// before the textures are uploaded (#202, #207), each with what its slot holds
/// (#1322).
pub(crate) fn material_map_paths(
    scene: &Scene,
    admit: impl Fn(u32) -> bool,
) -> Vec<(MapRef, TextureKind)> {
    scene
        .world
        .ids_with_material()
        .into_iter()
        .filter(|&id| admit(id))
        .flat_map(|id| scene.material_assets_of(id))
        .flat_map(|m| materials::material_maps(Some(m)))
        .collect()
}
