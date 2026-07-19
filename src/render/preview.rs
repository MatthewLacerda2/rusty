//! src/render/preview.rs — the Inspector's offscreen asset-preview target (#352).
//!
//! Sibling of `viewport.rs`: the Inspector's Preview tab renders a small, isolated
//! scene (a toggleable mesh + a hardcoded light, never the active editor World) into
//! its own offscreen colour texture, shown inside an `egui::Image` the same way the
//! Scene/Game viewport is. `resize_preview` sizes only this target — it never touches
//! `self.size`/the depth-post-FX chain the viewport owns, since the preview renders
//! through the same forward pass at whatever small panel size the Preview tab has,
//! independently of the main viewport's size.

use super::gpu::pipelines::create_pipelines;
use super::gpu::shaders::ShaderRegistry;
use super::{Camera, Renderer};
use crate::scene::Scene;

impl Renderer {
    /// An owned view of the offscreen preview target, or `None` before the first
    /// [`Self::resize_preview`]. See [`Self::viewport_target_view`] for why an owned
    /// `wgpu::TextureView` (not a borrow) is returned.
    pub fn preview_target_view(&self) -> Option<wgpu::TextureView> {
        self.preview_target
            .as_ref()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
    }

    /// (Re)allocate the offscreen preview colour target at `width` x `height` — the
    /// Preview tab's image rect in pixels — and resize the shared scene depth/post-FX
    /// targets to match, exactly as [`Self::resize_viewport`] does for the main
    /// viewport. The two share one `Renderer`, so whichever renders last in a frame
    /// leaves `self.size` at its own dimensions; the other resizes back to its own
    /// size the next time it runs (a cheap no-op reallocation, editor-only cost).
    /// A cheap no-op when the size is already current.
    pub fn resize_preview(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.size.width == width && self.size.height == height && self.preview_target.is_some() {
            return;
        }

        self.resize_scene_targets(width, height);

        let target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Inspector Preview Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.preview_target = Some(target);
    }

    /// Render `scene` with the forward pipeline temporarily rebuilt from
    /// `shader_path`'s compiled module instead of `assets/shaders/shader.wgsl` — the
    /// Preview tab's Shader-asset arm ("the chosen preview mesh... shaded by the
    /// selected module", #352). Reuses [`create_pipelines`] verbatim (same bind-group
    /// layouts, same vertex/fragment entry points as every other forward pipeline) so
    /// this is a swapped shader module, not a new pipeline shape. Falls back to the
    /// engine's default shader — rather than panicking — when `shader_path` can't be
    /// read or fails to compose, so a bad or half-written file still previews
    /// *something* instead of crashing the live editor.
    pub fn render_preview_with_shader(
        &mut self,
        scene: &Scene,
        camera: &Camera,
        view: &wgpu::TextureView,
        shader_path: &str,
    ) {
        let Some(module) = self.compose_preview_shader(shader_path) else {
            self.render(scene, camera, view, false, &[]);
            return;
        };
        let pipelines = create_pipelines(
            &self.device,
            &module,
            self.config.format,
            &self.camera_lighting_layout,
            &self.entity_bones_layout,
            &self.material_layout,
            &self.shadow_layout,
        );
        // Only the opaque forward pipeline is needed: the preview material is always
        // `MaterialAsset::default()` (Opaque), so the transparent/line/outline
        // pipelines never draw in this pass (`render(.., editor_mode: false, ..)`
        // skips the editor-only outline/grid overlays).
        let previous = std::mem::replace(&mut self.render_pipeline, pipelines.forward);
        self.render(scene, camera, view, false, &[]);
        self.render_pipeline = previous;
    }

    /// Read and compose `shader_path` against the engine's one shared `common.wgsl`
    /// (`assets/shaders/common.wgsl` — every shader's `#import "common"` target,
    /// regardless of where the previewed file itself lives), returning `None` rather
    /// than panicking on a missing file or a composition error (the non-panicking
    /// path `ShaderRegistry` already exposes for the #272 authoring bake).
    fn compose_preview_shader(&self, shader_path: &str) -> Option<wgpu::ShaderModule> {
        let source = std::fs::read_to_string(shader_path).ok()?;
        let mut composer = ShaderRegistry::composer_with_common("assets/shaders").ok()?;
        let naga_module = ShaderRegistry::validate_source(&mut composer, &source).ok()?;
        Some(
            self.device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("Preview Shader"),
                    source: wgpu::ShaderSource::Naga(std::borrow::Cow::Owned(naga_module)),
                }),
        )
    }
}

#[cfg(test)]
#[path = "preview_tests.rs"]
mod preview_tests;
