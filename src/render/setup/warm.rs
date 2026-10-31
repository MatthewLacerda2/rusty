//! Building the pipelines that otherwise wait for their first draw (#985).
//!
//! Headless renderers (one per GPU test) leave them lazy, so a test pays only for the
//! passes it draws. A windowed renderer is a game or the editor, where compiling a
//! pipeline on the first muzzle flash or smoke cloud is a hitch mid-combat, so
//! [`Renderer::new`] warms them all before the first frame.

use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::Renderer;

impl Renderer {
    /// Build every pipeline [`Self::lazy_pipelines`] lists, the editor's overlays
    /// only with `editor_overlays` (a shipped game draws none), and have each
    /// authored surface shader build its facing pipelines with it from now on: its
    /// slot is built on its first draw, after the renderer starts (#1615).
    pub(crate) fn warm_pipelines(&mut self, editor_overlays: bool) {
        self.surface_shaders.warm();
        for pipeline in self.lazy_pipelines(editor_overlays) {
            pipeline.get();
        }
    }

    /// Every pipeline built on first draw. A new [`LazyPipeline`] goes in this list,
    /// or the windowed renderer compiles it mid-frame.
    pub(crate) fn lazy_pipelines(&self, editor_overlays: bool) -> Vec<&LazyPipeline> {
        // The opaque pass and its prepass built their usual facing with the renderer;
        // their mirrored and double-sided ones (#1615) wait like the rest.
        let mut all: Vec<&LazyPipeline> = self.render_pipeline.twins().collect();
        all.extend(self.prepass_pipeline.twins());
        all.extend(self.primed_pipelines.lazies());
        all.extend(self.transparent_pipelines.lazies());
        all.extend(self.depth_postpass_pipelines.lazies());
        all.extend(self.velocity_pipeline.lazies());
        all.extend(self.post_passes.lazy_pipelines());
        all.extend(self.ssao.lazy_pipelines());
        all.extend(self.skybox_renderer.lazy_pipelines());
        all.extend(self.particle_renderer.lazy_pipelines());
        all.extend(self.ribbon_renderer.lazy_pipelines());
        all.extend(self.ui_renderer.lazy_pipelines());
        all.extend(self.shadow_renderer.lazy_pipelines());
        all.push(self.mip_generator.lazy_pipeline());
        all.push(self.opaque_copy.lazy_pipeline());
        // The authored surface shaders built so far: their facing pipelines (#1615).
        all.extend(self.surface_shaders.lazy_pipelines());
        if editor_overlays {
            all.push(&self.line_pipeline);
            all.extend(self.outline.lazy_pipelines());
        }
        all
    }
}
