//! Building the pipelines that otherwise wait for their first draw (#985).
//!
//! Headless renderers (one per GPU test) leave them lazy, so a test pays only for the
//! passes it draws. A windowed renderer is a game or the editor, where compiling a
//! pipeline on the first muzzle flash or smoke cloud is a hitch mid-combat, so
//! [`Renderer::new`] warms them all before the first frame.

use crate::render::gpu::pipelines::LazyPipeline;
use crate::render::Renderer;

impl Renderer {
    /// Build every lazy pipeline now: the translucent, particle, ribbon, world-canvas,
    /// mirrored-shadow and data-mip ones always, the debug-line and selection-outline (mask and edge) ones only
    /// with `editor_overlays` (a shipped game draws neither).
    pub(crate) fn warm_pipelines(&self, editor_overlays: bool) {
        for pipeline in self.lazy_pipelines(editor_overlays) {
            pipeline.get();
        }
    }

    /// Every pipeline built on first draw. A new [`LazyPipeline`] goes in this list,
    /// or the windowed renderer compiles it mid-frame.
    pub(crate) fn lazy_pipelines(&self, editor_overlays: bool) -> Vec<&LazyPipeline> {
        let mut all = vec![&self.transparent_pipeline];
        all.extend(self.particle_renderer.lazy_pipelines());
        all.extend(self.ribbon_renderer.lazy_pipelines());
        all.extend(self.ui_renderer.lazy_pipelines());
        all.extend(self.shadow_renderer.lazy_pipelines());
        all.push(self.mip_generator.lazy_pipeline());
        if editor_overlays {
            all.push(&self.line_pipeline);
            all.extend(self.outline.lazy_pipelines());
        }
        all
    }
}
