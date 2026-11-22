use crate::gpu::pipelines::{LazyCompute, LazyPipeline};
use crate::Renderer;

impl Renderer {
    pub fn warm_pipelines(&mut self, editor_overlays: bool) {
        self.surface_shaders.warm();
        for pipeline in self.lazy_pipelines(editor_overlays) {
            pipeline.get();
        }
        for pipeline in self.lazy_computes() {
            pipeline.get();
        }
    }

    pub(crate) fn lazy_computes(&self) -> Vec<&LazyCompute> {
        let mut all = self.post_passes.lazy_computes();
        all.extend(self.hiz.lazy_computes());
        all.extend(self.particle_renderer.gpu.lazy_computes());
        all
    }

    pub(crate) fn lazy_pipelines(&self, editor_overlays: bool) -> Vec<&LazyPipeline> {
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
        all.extend(self.mip_generator.lazy_pipelines());
        all.push(self.opaque_copy.lazy_pipeline());
        all.extend(self.surface_shaders.lazy_pipelines());
        if editor_overlays {
            all.push(&self.line_pipeline);
            all.push(&self.vertex_line_pipeline);
            all.push(&self.probe_ball_pipeline);
            all.push(self.navmesh.pipeline());
            all.extend(self.outline.lazy_pipelines());
        }
        all
    }
}
