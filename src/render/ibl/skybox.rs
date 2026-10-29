use crate::render::gpu::mesh::vertex_layout;
use crate::render::gpu::pipelines::{velocity_target, LazyPipeline};
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::gpu::DEPTH_FORMAT;
use crate::render::postfx::effects::ssr::surface;
use crate::render::GpuTexture;
use wgpu::util::DeviceExt;

/// One sky shader and its layout: what its pipelines, with and without the velocity
/// target, are built from.
struct SkyKind {
    label: &'static str,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
}

impl SkyKind {
    /// An interior-facing, depth-test-only far-plane box pipeline from the shader's
    /// `vs_main`/`fs_main`, depth writes off; with `velocity`, into a view's velocity
    /// target too (#1435), and with `surface` its SSR surface target (#1526), which the
    /// sky leaves as cleared: no SSR.
    fn pipeline(
        &self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        (velocity, surface): (bool, bool),
    ) -> wgpu::RenderPipeline {
        let colour = Some(wgpu::ColorTargetState {
            format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        });
        let targets = [
            colour,
            Some(velocity_target(wgpu::ColorWrites::ALL)),
            Some(surface::target_state(wgpu::ColorWrites::empty())),
        ];
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(self.label),
            layout: Some(&self.layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[Some(vertex_layout())],
            },
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &targets[..1 + usize::from(velocity) + usize::from(surface)],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Render the interior faces
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH_FORMAT,
                depth_write_enabled: Some(false), // Don't write to depth buffer
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }
}

/// A sky pipeline layout over `groups`.
fn box_layout(
    device: &wgpu::Device,
    label: &str,
    groups: &[&wgpu::BindGroupLayout],
) -> wgpu::PipelineLayout {
    device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &groups.iter().copied().map(Some).collect::<Vec<_>>(),
        immediate_size: 0,
    })
}

/// The colour targets a scene pass writes besides the HDR one: the velocity target
/// (#1435), and with it the SSR surface target (#1526).
#[derive(Clone, Copy)]
pub(crate) struct SkyTargets {
    pub velocity: bool,
    pub surface: bool,
}

pub struct SkyboxRenderer {
    /// Panorama pipeline: samples a bound 2D equirectangular texture (group 1).
    pipeline: wgpu::RenderPipeline,
    /// Procedural fallback pipeline: a sky->horizon->ground gradient derived from the
    /// ambient term, drawn when no panorama is bound (#256). Needs only group 0.
    gradient_pipeline: wgpu::RenderPipeline,
    /// The same two writing a motion-blurred view's velocity target too (#1435): the
    /// sky moves with the camera's rotation. Built the first time such a view draws.
    /// The second pair also carries an SSR view's surface target (#1526).
    velocity: [[LazyPipeline; 2]; 2],
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    num_indices: u32,
}

impl SkyboxRenderer {
    pub fn new(
        device: &wgpu::Device,
        texture_layout: &wgpu::BindGroupLayout,
        camera_lighting_layout: &wgpu::BindGroupLayout,
        texture_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> Self {
        // The panorama reads camera + lighting and its texture; the procedural
        // gradient (#256) reads camera + ambient from group 0 only.
        let panorama = SkyKind {
            label: "Skybox Pipeline",
            shader: registry.load(device, "skybox.wgsl", "Skybox Shader"),
            layout: box_layout(
                device,
                "Skybox Pipeline",
                &[camera_lighting_layout, texture_layout],
            ),
        };
        let gradient = SkyKind {
            label: "Sky Gradient Pipeline",
            shader: registry.load(device, "sky_gradient.wgsl", "Sky Gradient Shader"),
            layout: box_layout(device, "Sky Gradient Pipeline", &[camera_lighting_layout]),
        };
        let pipeline = panorama.pipeline(device, texture_format, (false, false));
        let gradient_pipeline = gradient.pipeline(device, texture_format, (false, false));
        let kinds = std::sync::Arc::new([panorama, gradient]);
        let velocity = [false, true].map(|surface| {
            [0, 1].map(|kind| {
                let kinds = kinds.clone();
                LazyPipeline::new(device, move |device| {
                    kinds[kind].pipeline(device, texture_format, (true, surface))
                })
            })
        });
        let (vertex_buffer, index_buffer, num_indices) = Self::create_box_buffers(device);

        Self {
            pipeline,
            gradient_pipeline,
            velocity,
            vertex_buffer,
            index_buffer,
            num_indices,
        }
    }

    /// The pipelines built on first draw (`Renderer::lazy_pipelines`).
    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.velocity.iter().flatten()
    }

    /// Generate the unit box vertex/index buffers the skybox is drawn with.
    fn create_box_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        let (vertices, indices) = crate::components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skybox Vertex Buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Skybox Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        (vertex_buffer, index_buffer, indices.len() as u32)
    }

    /// Draw the panorama sky into a pass with `targets`.
    pub(crate) fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
        skybox_texture: &'a GpuTexture,
        targets: SkyTargets,
    ) {
        let pipeline = self.pick(&self.pipeline, 0, targets);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, camera_lighting_bind_group, &[]);
        render_pass.set_bind_group(1, &skybox_texture.bind_group, &[]);
        self.draw_box(render_pass);
    }

    /// Sky `kind`'s pipeline (0 the panorama, 1 the gradient) into `targets`;
    /// `colour_only` when the pass writes only the HDR target.
    fn pick<'a>(
        &'a self,
        colour_only: &'a wgpu::RenderPipeline,
        kind: usize,
        targets: SkyTargets,
    ) -> &'a wgpu::RenderPipeline {
        match targets {
            SkyTargets {
                velocity: false, ..
            } => colour_only,
            SkyTargets { surface, .. } => self.velocity[usize::from(surface)][kind].get(),
        }
    }

    /// Draw the procedural gradient backdrop (#256). Uses only group 0 (camera +
    /// ambient); the same far-plane box fills just the pixels no geometry covered.
    pub(crate) fn draw_gradient<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
        targets: SkyTargets,
    ) {
        let pipeline = self.pick(&self.gradient_pipeline, 1, targets);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, camera_lighting_bind_group, &[]);
        self.draw_box(render_pass);
    }

    /// Bind the shared box buffers and issue the indexed draw (panorama + gradient).
    fn draw_box<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}
