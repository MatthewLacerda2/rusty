use super::gpu::pipelines::create_pipelines;
use super::{RenderView, Renderer};
use rusty_scene::Camera;
use rusty_scene::Scene;
use rusty_shadergen::{baked, compose, engine_shader_dir};

impl Renderer {
    pub fn render_preview_with_shader(
        &mut self,
        view: &mut RenderView,
        scene: &Scene,
        camera: &Camera,
        output: &wgpu::TextureView,
        shader_path: &str,
    ) {
        let Some(module) = self.compose_preview_shader(shader_path) else {
            self.render(view, scene, camera, output, false);
            return;
        };
        let pipelines = create_pipelines(
            &self.device,
            &module,
            [super::postfx::HDR_FORMAT, self.config.format],
            &self.camera_lighting_layout,
            &self.entity_bones_layout,
            &self.material_layout,
            &self.shadow_layout,
        );
        view.set_forward_override(Some(pipelines.forward));
        self.render(view, scene, camera, output, false);
        view.set_forward_override(None);
    }

    fn compose_preview_shader(&self, shader_path: &str) -> Option<wgpu::ShaderModule> {
        let source = baked::load(std::path::Path::new(shader_path)).ok()?.source;
        let mut composer = compose::composer_with_common(engine_shader_dir()).ok()?;
        let naga_module = compose::compose(&mut composer, &source, shader_path).ok()?;
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

#[cfg(test)]
#[path = "shader_time_tests.rs"]
mod shader_time_tests;
