use super::cascades::MAX_CASCADES;
use super::layouts::{caster_entries, clip_layout, global_group, global_layout};
use super::ShadowRenderer;
use crate::gpu::groups::Groups;
use crate::gpu::mesh::vertex_layout;
use crate::gpu::pipelines::winding::Facing;
use crate::gpu::pipelines::LazyPipeline;

pub(super) struct DepthTextures {
    pub static_texture: wgpu::Texture,
    pub active_texture: wgpu::Texture,
    pub static_layers: Vec<wgpu::TextureView>,
    pub active_layers: Vec<wgpu::TextureView>,
    pub active_view: wgpu::TextureView,
}

impl ShadowRenderer {
    pub(super) fn create_depth_textures(device: &wgpu::Device) -> DepthTextures {
        let desc = wgpu::TextureDescriptor {
            label: Some("Shadow Cascade Depth Array"),
            size: wgpu::Extent3d {
                width: Self::CASCADE_SIZE,
                height: Self::CASCADE_SIZE,
                depth_or_array_layers: MAX_CASCADES as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        let layers = |texture: &wgpu::Texture| -> Vec<wgpu::TextureView> {
            (0..MAX_CASCADES as u32)
                .map(|layer| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        base_array_layer: layer,
                        array_layer_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect()
        };

        let static_texture = device.create_texture(&desc);
        let active_texture = device.create_texture(&desc);
        let active_view = active_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        DepthTextures {
            static_layers: layers(&static_texture),
            active_layers: layers(&active_texture),
            static_texture,
            active_texture,
            active_view,
        }
    }

    pub(super) fn create_sampler(device: &wgpu::Device) -> wgpu::Sampler {
        device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Shadow Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            // lint: allow(depth-direction) a shadow map keeps ordinary depth, its own short range
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        })
    }

    pub(super) fn create_pass_layouts(
        device: &wgpu::Device,
        [light_space_buffer, time_buffer]: [&wgpu::Buffer; 2],
    ) -> (
        wgpu::BindGroupLayout,
        wgpu::BindGroup,
        wgpu::BindGroupLayout,
    ) {
        let global_layout = global_layout(device);
        let global_bind_group =
            global_group(device, &global_layout, light_space_buffer, time_buffer);

        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Shadow Caster Layout"),
            entries: &caster_entries(),
        });

        (global_layout, global_bind_group, entity_layout)
    }

    pub(super) fn create_pipelines(
        device: &wgpu::Device,
        [global, entity, material]: [&wgpu::BindGroupLayout; 3],
        shader: &wgpu::ShaderModule,
    ) -> ([wgpu::RenderPipeline; 2], [[LazyPipeline; 2]; 2]) {
        let plain = Groups {
            global,
            scene: None,
            material: None,
            draw: entity,
        };
        let plain = plain.layout(device, "Shadow Pipeline Layout");
        let clip = clip_layout(device, [global, entity, material]);
        let twins = |facing| {
            [
                lazy_depth_pipeline(device, shader, &plain, None, facing),
                lazy_depth_pipeline(device, shader, &clip, Some("fs_shadow"), facing),
            ]
        };
        (
            [
                depth_pipeline(device, shader, &plain, None, Facing::Ccw),
                depth_pipeline(device, shader, &clip, Some("fs_shadow"), Facing::Ccw),
            ],
            [twins(Facing::Cw), twins(Facing::Both)],
        )
    }
}

pub(crate) fn lazy_depth_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: Option<&'static str>,
    facing: Facing,
) -> LazyPipeline {
    let (shader, layout) = (shader.clone(), layout.clone());
    LazyPipeline::new(device, move |device| {
        depth_pipeline(device, &shader, &layout, fragment, facing)
    })
}

/// Culls back faces, as Unity does: culling front ones would store the far side, light a strip
/// under every base, and drop single-sided planes. `Cw` keeps a mirrored caster's lit side.
pub(crate) fn depth_pipeline(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    fragment: Option<&str>,
    facing: Facing,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(match (fragment, facing) {
            (None, Facing::Ccw) => "Shadow Render Pipeline",
            (None, Facing::Cw) => "Shadow Mirrored Pipeline",
            (None, Facing::Both) => "Shadow Two-Sided Pipeline",
            (Some(_), Facing::Ccw) => "Shadow Clip Pipeline",
            (Some(_), Facing::Cw) => "Shadow Mirrored Clip Pipeline",
            (Some(_), Facing::Both) => "Shadow Two-Sided Clip Pipeline",
        }),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("vs_shadow"),
            compilation_options: Default::default(),
            buffers: &[Some(vertex_layout())],
        },
        fragment: fragment.map(|entry_point| wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry_point),
            compilation_options: Default::default(),
            targets: &[],
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: facing.front_face(),
            cull_mode: facing.cull_mode(),
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            // lint: allow(depth-direction) a shadow map keeps ordinary depth, its own short range
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            // Hardware slope bias against acne, beside the receiver's normal offset.
            bias: wgpu::DepthBiasState {
                constant: 2,
                slope_scale: 2.0,
                clamp: 0.0,
            },
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    })
}
