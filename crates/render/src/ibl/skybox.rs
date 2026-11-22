use crate::gpu::mesh::vertex_layout;
use crate::gpu::pipelines::{velocity_target, LazyPipeline};
use crate::gpu::shaders::ShaderRegistry;
use crate::gpu::DEPTH_FORMAT;
use crate::postfx::effects::ssr::surface;
use crate::GpuTexture;
use wgpu::util::DeviceExt;

const CAMERA: u32 = 0;
const SKY: u32 = 1;

struct SkyKind {
    label: &'static str,
    shader: wgpu::ShaderModule,
    layout: wgpu::PipelineLayout,
}

impl SkyKind {
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
                targets: &targets[..1 + usize::from(velocity) + 2 * usize::from(surface)],
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

#[derive(Clone, Copy)]
pub(crate) struct SkyTargets {
    pub velocity: bool,
    pub surface: bool,
}

pub struct SkyboxRenderer {
    pipeline: wgpu::RenderPipeline,
    gradient_pipeline: wgpu::RenderPipeline,
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

    pub(crate) fn lazy_pipelines(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.velocity.iter().flatten()
    }

    fn create_box_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        let (vertices, indices) = rusty_components::mesh::primitives::generate_box(2.0, 2.0, 2.0);
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

    pub(crate) fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
        skybox_texture: &'a GpuTexture,
        targets: SkyTargets,
    ) {
        let pipeline = self.pick(&self.pipeline, 0, targets);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(CAMERA, camera_lighting_bind_group, &[]);
        render_pass.set_bind_group(SKY, &skybox_texture.bind_group, &[]);
        self.draw_box(render_pass);
    }

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

    pub(crate) fn draw_gradient<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
        targets: SkyTargets,
    ) {
        let pipeline = self.pick(&self.gradient_pipeline, 1, targets);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(CAMERA, camera_lighting_bind_group, &[]);
        self.draw_box(render_pass);
    }

    fn draw_box<'a>(&'a self, render_pass: &mut wgpu::RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
    }
}
