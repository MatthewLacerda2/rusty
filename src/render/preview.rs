//! src/render/preview.rs — the Inspector's Shader-asset preview pass (#352).
//!
//! The Inspector's Preview tab renders a small, isolated scene (a toggleable mesh + a
//! hardcoded light, never the active editor World) into its own [`RenderView`], shown
//! inside an `egui::Image` the same way the Scene/Game viewport is. That view (its
//! offscreen target, depth buffer, and post-FX chain) is owned by the front-end and
//! independent of the main viewport's, so the two never fight over shared targets
//! (#355). This module carries only the Shader-asset arm: rendering the preview with
//! the forward pipeline swapped for a compiled `.wgsl` module.

use super::gpu::pipelines::create_pipelines;
use super::gpu::shaders::ShaderRegistry;
use super::{Camera, RenderView, Renderer};
use crate::scene::Scene;

impl Renderer {
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
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
        shader_path: &str,
    ) {
        let Some(module) = self.compose_preview_shader(shader_path) else {
            self.render(view, scene, camera, output, false, &[]);
            return;
        };
        let pipelines = create_pipelines(
            &self.device,
            &module,
            super::postfx::HDR_FORMAT,
            &self.camera_lighting_layout,
            &self.entity_bones_layout,
            &self.material_layout,
            &self.shadow_layout,
        );
        // Only the opaque forward pipeline is needed: the preview material is always
        // `MaterialAsset::default()` (Opaque), so the transparent/line/outline
        // pipelines never draw in this pass (`render(.., editor_mode: false, ..)`
        // skips the editor-only outline/grid overlays).
        //
        // The override rides on the *view*, not on the shared renderer (#355 step 4):
        // this used to swap `Renderer::render_pipeline` out and put it back around the
        // call, which leaves the whole editor shaded by a preview module if anything
        // in between returns early or unwinds. A view-owned override cannot leak.
        view.set_forward_override(Some(pipelines.forward));
        self.render(view, scene, camera, output, false, &[]);
        view.set_forward_override(None);
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
