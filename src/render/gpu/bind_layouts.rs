//! Bind-group-layout builders for the forward renderer's four groups. Pure
//! constructors split out of `pipelines.rs` to keep each file under the size cap;
//! `pipelines` consumes these to build the pipeline layouts (behavior unchanged).

use crate::render::draw::materials::MAP_BINDINGS;
use crate::render::passes::transparent::OpaqueViews;

use super::layout_entries::{
    depth_entry, dynamic_uniform_entry, sampler_entry, storage_entry, texture_entry, uniform_entry,
};

/// Group 0: Camera (0), Lighting (1), the sky's reflection cube (2) and its sampler (3,
/// #1553), Reflection
/// Probe Cube Array (4), Reflection Probe Sampler (5), the light clusters (#434): the
/// frame's point/spot lights (7), each cluster's light and decal ranges (8) and the
/// index list (9), and the decals (#638): their records (10), the decal atlas's
/// sRGB (11) and raw (12) views and its sampler (13), and the baked lightmap pages (14)
/// with their sampler (15, #438) and direction pages (16, #810), and the
/// integrated volumetric fog (17) with its sampler (18, #1703). Binding 6 is the shadow module's (see `shader.wgsl`).
/// The cube array holds the resident probes' baked, prefiltered cubemaps, one per layer
/// (#245, #1206); a draw's instance names the two probes it blends (#1584), and the
/// shader samples their layers (parallax-corrected, roughness->mip) and the sky's
/// cube for the weight they leave.
pub(crate) fn create_camera_lighting_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Camera, Lighting, Skybox & Reflection Layout"),
        entries: &camera_lighting_entries(),
    })
}

/// [`create_camera_lighting_layout`]'s entries.
pub(crate) fn camera_lighting_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use wgpu::TextureViewDimension::{Cube, CubeArray, D2Array, D3};
    vec![
        uniform_entry(0, wgpu::ShaderStages::VERTEX_FRAGMENT),
        // Vertex-visible too: lit particles shade per vertex (#440).
        uniform_entry(1, wgpu::ShaderStages::VERTEX_FRAGMENT),
        // The sky's prefiltered reflection cube (#1553).
        texture_entry(2, Cube),
        sampler_entry(3),
        texture_entry(4, CubeArray),
        sampler_entry(5),
        // Vertex-visible too, for lit particles (#440).
        storage_entry(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(8, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(9, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(10, wgpu::ShaderStages::FRAGMENT),
        texture_entry(11, D2Array),
        texture_entry(12, D2Array),
        sampler_entry(13),
        texture_entry(14, D2Array),
        sampler_entry(15),
        texture_entry(16, D2Array),
        // The integrated volumetric fog (#1703); vertex-visible too: particles
        // fog per vertex.
        wgpu::BindGroupLayoutEntry {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ..texture_entry(17, D3)
        },
        wgpu::BindGroupLayoutEntry {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ..sampler_entry(18)
        },
        // The shadowmask pages (#1695), beside the lightmaps.
        texture_entry(19, D2Array),
    ]
}

/// The scene depth target as a sampled texture (binding 0), for soft particles.
pub(crate) fn create_scene_depth_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Scene Depth Layout"),
        entries: &[depth_entry(0, wgpu::TextureViewDimension::D2)],
    })
}

/// Group 1 (#470): the per-draw uniform (0) at a dynamic offset into the frame's
/// packed uniforms, the joint-matrix array (1) the vertex shader indexes from the
/// uniform's `bone_base` (#455), the instance array (2) it indexes by
/// `instance_index`, the material records (4) the uniform's `material` indexes
/// (#1616; binding 3 is the shadow casters', in the same module), the Light Probe
/// Proxy Volumes (5) an instance's `proxy_volume` indexes (#1672), and the object
/// records (6) an instance's `object` names (#1706). One bind group serves every
/// solid draw; only the offset changes between batches.
///
/// The instances are the vertex stage's alone: it hands the fragment stage the
/// object, reflection probe and proxy volume it needs as a flat varying, which keeps
/// the fragment stage at `wgpu::Limits::default()`'s 8 storage buffers (4 in group 0,
/// materials, proxies and objects here, the shadow tiles in group 3), the limit the
/// renderer requests on every backend. The vertex stage binds 7.
pub(crate) fn create_entity_bones_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Entity & Bones Layout"),
        entries: &entity_bones_entries(),
    })
}

/// [`create_entity_bones_layout`]'s entries.
pub(crate) fn entity_bones_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use crate::render::DrawUniform;
    vec![
        dynamic_uniform_entry(
            0,
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            std::mem::size_of::<DrawUniform>() as u64,
        ),
        storage_entry(1, wgpu::ShaderStages::VERTEX),
        storage_entry(2, wgpu::ShaderStages::VERTEX),
        storage_entry(4, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(5, wgpu::ShaderStages::FRAGMENT),
        storage_entry(6, wgpu::ShaderStages::VERTEX_FRAGMENT),
    ]
}

/// Group 2 (single-texture): Texture (0) & Sampler (1). Kept for `GpuTexture`'s own
/// bind group, used by the particle/skybox passes that bind one texture each.
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
    texture_entry(binding, wgpu::TextureViewDimension::D2)
}

/// A filtering sampler bind-group-layout entry at `binding`, FRAGMENT-visible.
fn material_sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

/// The detail map's group-2 binding: `materials::MAP_BINDINGS`' last.
const DETAIL_BINDING: u32 = MAP_BINDINGS[5];

// Every extra shader slot binds below the detail map, and every texture below the
// samplers' offset, so no two group-2 bindings meet.
const _: () = {
    let mut i = 0;
    while i < crate::shadergen::textures::SLOTS.len() {
        assert!(crate::shadergen::textures::SLOTS[i].binding < DETAIL_BINDING);
        i += 1;
    }
    assert!(DETAIL_BINDING < crate::shadergen::textures::SAMPLER_OFFSET);
};

/// Group 2 (one per distinct material map set, #470): the maps at `MAP_BINDINGS`,
/// albedo (0) and its sampler (1), metallic (2), roughness (3), normal (4) and
/// emissive (5); the material's runtime shader params (6, #399), a fixed
/// `array<vec4<f32>, 16>` uniform only a surface variant with runtime params reads;
/// one texture per extra shader slot (7 = `mask`, #400), which only a variant with a
/// block sampling it reads; and the detail map (15, #1997). Every texture but
/// albedo has its own sampler at `textures::sampler_binding` (16 past it), the wrap
/// and filter of its import settings (#1323). This is the layout the forward pass's
/// group(2) is built against (#202, #207).
pub(crate) fn create_material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Material Layout"),
        entries: &material_entries(),
    })
}

/// [`create_material_layout`]'s entries.
pub(crate) fn material_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use crate::shadergen::textures::{sampler_binding, SLOTS};
    let params = wgpu::BindGroupLayoutEntry {
        binding: crate::shadergen::params::PARAM_BINDING,
        // The vertex stage too: a vertex block's runtime param (#1524).
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(PARAMS_SIZE),
        },
        count: None,
    };
    let textures = MAP_BINDINGS
        .into_iter()
        .chain(SLOTS.iter().map(|s| s.binding));
    let mut entries = vec![material_sampler_entry(1), params];
    for binding in textures {
        entries.push(material_texture_entry(binding));
        if binding != MAP_BINDINGS[0] {
            entries.push(material_sampler_entry(sampler_binding(binding)));
        }
    }
    entries
}

/// Main shadow bind group layout: the cascade uniform, the cascade depth array, the
/// comparison sampler (#435), the view's ambient-occlusion texture (#436), and the
/// point/spot shadow atlas with its tiles (#468), and the opaque copy a refracting
/// or absorbing surface reads (#1523: colour and eye distance, and its sampler) — the
/// forward pass's group 3, everything it reads that a per-frame pass produced.
pub(crate) fn create_shadow_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Main Shadow Bind Group Layout"),
        entries: &shadow_entries(),
    })
}

/// [`create_shadow_layout`]'s entries.
pub(crate) fn shadow_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use wgpu::TextureViewDimension::{D2Array, D2};
    vec![
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
        texture_entry(6, D2),
        sampler_entry(7),
    ]
}

/// The forward pass's group 3 over [`create_shadow_layout`]: the cascade `uniform`,
/// the shadow renderer's active cascade array + sampler, `ao` — a view's SSAO
/// result, or the 1x1 white fallback where no AO ran — and `opaque`, the view's
/// opaque copy in its transparent pass or the 1x1 fallback everywhere else (#1523).
pub(crate) fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    shadows: &crate::render::passes::shadows::ShadowRenderer,
    ao: &wgpu::TextureView,
    opaque: OpaqueViews,
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
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::TextureView(opaque.texture),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::Sampler(opaque.sampler),
            },
        ],
    })
}
