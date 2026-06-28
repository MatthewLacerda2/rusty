use crate::render::gpu::mesh::Vertex;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::GpuTexture;
use wgpu::util::DeviceExt;

pub struct SkyboxRenderer {
    /// Panorama pipeline: samples a bound 2D equirectangular texture (group 1).
    pipeline: wgpu::RenderPipeline,
    /// Procedural fallback pipeline: a sky->horizon->ground gradient derived from the
    /// ambient term, drawn when no panorama is bound (#256). Needs only group 0.
    gradient_pipeline: wgpu::RenderPipeline,
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
        let pipeline = Self::create_pipeline(
            device,
            texture_layout,
            camera_lighting_layout,
            texture_format,
            registry,
        );
        let gradient_pipeline = Self::create_gradient_pipeline(
            device,
            camera_lighting_layout,
            texture_format,
            registry,
        );
        let (vertex_buffer, index_buffer, num_indices) = Self::create_box_buffers(device);

        Self {
            pipeline,
            gradient_pipeline,
            vertex_buffer,
            index_buffer,
            num_indices,
        }
    }

    /// Build an interior-facing, depth-test-only box pipeline from `shader`'s
    /// `vs_main`/`fs_main`, against `bind_group_layouts`. Shared by the panorama and
    /// procedural-gradient skybox variants — both draw the same far-plane box with
    /// depth writes off, differing only in shader and bind groups.
    fn create_box_pipeline(
        device: &wgpu::Device,
        label: &str,
        shader: &wgpu::ShaderModule,
        bind_group_layouts: &[&wgpu::BindGroupLayout],
        texture_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts,
            push_constant_ranges: &[],
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: texture_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Render the interior faces
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Don't write to depth buffer
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        })
    }

    /// Build the interior-facing, depth-test-only panorama skybox pipeline.
    fn create_pipeline(
        device: &wgpu::Device,
        texture_layout: &wgpu::BindGroupLayout,
        camera_lighting_layout: &wgpu::BindGroupLayout,
        texture_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> wgpu::RenderPipeline {
        let shader = registry.load(device, "skybox.wgsl", "Skybox Shader");
        Self::create_box_pipeline(
            device,
            "Skybox Pipeline",
            &shader,
            &[camera_lighting_layout, texture_layout],
            texture_format,
        )
    }

    /// Build the procedural-gradient fallback pipeline (#256). It reads camera +
    /// ambient from group 0 only (no texture), so its layout is just the
    /// camera/lighting group.
    fn create_gradient_pipeline(
        device: &wgpu::Device,
        camera_lighting_layout: &wgpu::BindGroupLayout,
        texture_format: wgpu::TextureFormat,
        registry: &mut ShaderRegistry,
    ) -> wgpu::RenderPipeline {
        let shader = registry.load(device, "sky_gradient.wgsl", "Sky Gradient Shader");
        Self::create_box_pipeline(
            device,
            "Sky Gradient Pipeline",
            &shader,
            &[camera_lighting_layout],
            texture_format,
        )
    }

    /// Generate the unit box vertex/index buffers the skybox is drawn with.
    fn create_box_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
        let (vertices, indices) = crate::render::gpu::mesh::generate_box(2.0, 2.0, 2.0);
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

    pub fn draw<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
        skybox_texture: &'a GpuTexture,
    ) {
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, camera_lighting_bind_group, &[]);
        render_pass.set_bind_group(1, &skybox_texture.bind_group, &[]);
        self.draw_box(render_pass);
    }

    /// Draw the procedural gradient backdrop (#256). Uses only group 0 (camera +
    /// ambient); the same far-plane box fills just the pixels no geometry covered.
    pub fn draw_gradient<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        camera_lighting_bind_group: &'a wgpu::BindGroup,
    ) {
        render_pass.set_pipeline(&self.gradient_pipeline);
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
