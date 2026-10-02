//! Bind-group-layout builders for the forward renderer's four groups. Pure
//! constructors split out of `pipelines.rs` to keep each file under the size cap;
//! `pipelines` consumes these to build the pipeline layouts (behavior unchanged).

/// A uniform-buffer layout entry at `binding`, seen by `visibility`.
fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// A FRAGMENT-visible filterable texture layout entry at `binding`, of the given view
/// dimension (2D for the skybox, Cube for the reflection probe).
fn texture_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

/// A FRAGMENT-visible filtering-sampler layout entry at `binding`.
fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// Group 0: Camera (0), Lighting (1), Skybox Texture (2), Skybox Sampler (3), Reflection
/// Probe Cube (4), Reflection Probe Sampler (5), and the light clusters (#434): the
/// frame's point/spot lights (7), each cluster's light range (8) and the light-index
/// list (9). Binding 6 is the shadow module's (see `shader.wgsl`). The cube is the active probe's baked,
/// prefiltered cubemap (#245); the shader samples it (parallax-corrected, roughness->mip)
/// when a probe applies and falls back to the 2D skybox otherwise.
pub(crate) fn create_camera_lighting_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::TextureViewDimension::{Cube, D2};
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera, Lighting, Skybox & Reflection Layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
            // Vertex-visible too: lit particles shade per vertex (#440).
            uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
            texture_entry(2, D2),
            sampler_entry(3),
            texture_entry(4, Cube),
            sampler_entry(5),
            // Vertex-visible too, for lit particles (#440).
            storage_entry(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
            storage_entry(8, wgpu::ShaderStages::VERTEX_FRAGMENT),
            storage_entry(9, wgpu::ShaderStages::VERTEX_FRAGMENT),
        ],
    })
}

/// Group 1 (#470): the per-draw material uniform (0) at a dynamic offset into the
/// frame's packed uniforms, the joint-matrix array (1) the vertex shader indexes from
/// the uniform's `bone_base` (#455), and the instance array (2) it indexes by
/// `instance_index`. One bind group serves every solid draw; only the offset changes
/// between batches.
pub(crate) fn create_entity_bones_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use crate::render::EntityUniform;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Entity & Bones Layout"),
        entries: &[
            dynamic_uniform_entry(0, std::mem::size_of::<EntityUniform>()),
            storage_entry(1, wgpu::ShaderStages::VERTEX),
            storage_entry(2, wgpu::ShaderStages::VERTEX_FRAGMENT),
        ],
    })
}

/// A vertex+fragment uniform entry bound at a dynamic offset, `size` bytes wide.
fn dynamic_uniform_entry(binding: u32, size: usize) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: true,
            min_binding_size: wgpu::BufferSize::new(size as u64),
        },
        count: None,
    }
}

/// A read-only storage-buffer entry at `binding` (an instance array, #470).
pub(crate) fn storage_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only: true },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// Group 2 (single-texture): Texture (0) & Sampler (1). Kept for `GpuTexture`'s own
/// bind group, used by the particle/decal/skybox passes that bind one texture each.
pub(crate) fn create_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Texture Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

/// Bytes in a material's runtime shader-param uniform (#399).
pub(crate) const PARAMS_SIZE: u64 =
    std::mem::size_of::<crate::shadergen::params::PackedParams>() as u64;

/// A filterable 2D texture bind-group-layout entry at `binding`, FRAGMENT-visible.
fn material_texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// Group 2 (per-entity material): albedo (0), shared sampler (1), metallic map (2),
/// roughness map (3), normal map (4), emissive map (5), and the material's runtime
/// shader params (6, #399) — a fixed `array<vec4<f32>, 16>` uniform only a surface
/// variant with runtime params reads — then one texture per extra shader slot (7 =
/// `mask`, #400), which only a variant with a block sampling it reads. One sampler
/// (binding 1) services every texture. This is the layout the forward pass's
/// group(2) is built against (#202, #207).
pub(crate) fn create_material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    let mut entries = vec![
        material_texture_entry(0),
        wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
            count: None,
        },
        material_texture_entry(2),
        material_texture_entry(3),
        material_texture_entry(4),
        material_texture_entry(5),
        wgpu::BindGroupLayoutEntry {
            binding: crate::shadergen::params::PARAM_BINDING,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: wgpu::BufferSize::new(PARAMS_SIZE),
            },
            count: None,
        },
    ];
    let slots = crate::shadergen::textures::SLOTS.iter();
    entries.extend(slots.map(|slot| material_texture_entry(slot.binding)));
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Material Layout"),
        entries: &entries,
    })
}

/// Main shadow bind group layout: the cascade uniform, the cascade depth array, the
/// comparison sampler (#435), the view's ambient-occlusion texture (#436), and the
/// point/spot shadow atlas with its tiles (#468) — the forward pass's group 3,
/// everything it reads that a per-frame pass produced.
pub(crate) fn create_shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    use wgpu::TextureViewDimension::{D2Array, D2};
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Main Shadow Bind Group Layout"),
        entries: &[
            uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
            depth_entry(1, D2Array),
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                count: None,
            },
            texture_entry(3, D2),
            depth_entry(4, D2),
            storage_entry(5, wgpu::ShaderStages::FRAGMENT),
        ],
    })
}

/// A FRAGMENT-visible depth texture entry at `binding`, sampled by comparison.
fn depth_entry(binding: u32, dim: wgpu::TextureViewDimension) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Depth,
            view_dimension: dim,
            multisampled: false,
        },
        count: None,
    }
}

/// The forward pass's group 3 over [`create_shadow_layout`]: the cascade `uniform`,
/// the shadow renderer's active cascade array + sampler, and `ao` — a view's SSAO
/// result, or the 1x1 white fallback where no AO ran.
pub(crate) fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    shadows: &crate::render::passes::shadows::ShadowRenderer,
    ao: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Main Shadow Bind Group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&shadows.active_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&shadows.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(ao),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::TextureView(&shadows.atlas.view),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: shadows.atlas.tiles.as_entire_binding(),
            },
        ],
    })
}
