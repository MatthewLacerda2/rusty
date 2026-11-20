//! Bind-group-layout builders for the forward renderer's four groups. Pure
//! constructors split out of `pipelines.rs` to keep each file under the size cap;
//! `pipelines` consumes these to build the pipeline layouts (behavior unchanged).

use crate::draw::map_slots::all_bindings;
use crate::gpu::ops::CountedDevice as _;
use crate::passes::transparent::OpaqueViews;

use super::layout_entries::{
    depth_entry, dynamic_uniform_entry, sampler_entry, storage_entry, texture_entry, uniform_entry,
};

/// Group 0: Camera (0), Lighting (1), the sky's reflection cube (2, #1553), the
/// Reflection Probe Cube Array (4), the light clusters (#434): the
/// frame's point/spot lights (7), each cluster's light and decal ranges (8) and the
/// index list (9), and the decals (#638): their records (10), the decal atlas's
/// sRGB (11) and raw (12) views and its sampler (13), and the baked lightmap pages (14, #438)
/// with their direction pages (16, #810), the integrated volumetric fog (17, #1703)
/// and the shadowmask pages (19). Bindings 3, 5 and 15 are gone: 18 is the one
/// linear-clamp sampler the sky, the probes, the lightmaps, the shadowmask pages
/// and the fog volume all read. Binding 6 is the shadow module's (see `shader.wgsl`).
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
        // Bindings 3, 5 and 15 are free: the cubes and the lightmap pages read
        // through the one sampler at 18.
        texture_entry(2, Cube),
        texture_entry(4, CubeArray),
        // Vertex-visible too, for lit particles (#440).
        storage_entry(7, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(8, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(9, wgpu::ShaderStages::VERTEX_FRAGMENT),
        storage_entry(10, wgpu::ShaderStages::FRAGMENT),
        texture_entry(11, D2Array),
        texture_entry(12, D2Array),
        sampler_entry(13),
        texture_entry(14, D2Array),
        texture_entry(16, D2Array),
        // The integrated volumetric fog (#1703); vertex-visible too: particles
        // fog per vertex.
        wgpu::BindGroupLayoutEntry {
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ..texture_entry(17, D3)
        },
        // The linear clamp sampler every engine-owned texture here reads through
        // but the decal atlas (`gpu::linear_clamp`), the fog's among them.
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
        entries: &scene_depth_entries(),
    })
}

/// [`create_scene_depth_layout`]'s entries.
pub(crate) fn scene_depth_entries() -> [wgpu::BindGroupLayoutEntry; 1] {
    [depth_entry(0, wgpu::TextureViewDimension::D2)]
}

/// The per-draw group (#470): the per-draw uniform (0) at a dynamic offset into the frame's
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
/// materials, proxies and objects here, the shadow tiles in the scene group), the limit the
/// renderer requests on every backend. The vertex stage binds 7.
pub(crate) fn create_entity_bones_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Entity & Bones Layout"),
        entries: &entity_bones_entries(),
    })
}

/// [`create_entity_bones_layout`]'s entries.
pub(crate) fn entity_bones_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use crate::DrawUniform;
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

/// A single-texture group: Texture (0) & Sampler (1). Kept for `GpuTexture`'s own
/// bind group, used by the particle/skybox passes that bind one texture each.
pub(crate) fn create_texture_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Texture Layout"),
        entries: &texture_entries(),
    })
}

/// [`create_texture_layout`]'s entries.
pub(crate) fn texture_entries() -> [wgpu::BindGroupLayoutEntry; 2] {
    [
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
    ]
}

/// Bytes in a material's runtime shader-param uniform (#399).
pub(crate) const PARAMS_SIZE: u64 =
    std::mem::size_of::<rusty_shadergen::params::PackedParams>() as u64;

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

/// Group 2 (one per distinct material map set, #470): the maps at their
/// `map_slots::MAP_SLOTS` rows' bindings (no two textures meet, a const assert there),
/// albedo (0) and its sampler (1), metallic (2), roughness (3), normal (4) and
/// emissive (5); the material's runtime shader params (6, #399), a fixed
/// `array<vec4<f32>, 16>` uniform only a surface variant with runtime params reads;
/// one texture per extra shader slot (7 = `mask`, #400), which only a variant with a
/// block sampling it reads; the detail map (15, #1997), the occlusion (14) and
/// specular-level (13) maps (#2151) the height map (12, #2191) and the detail mask (11, #2258). Every texture but albedo has its own sampler at
/// `textures::sampler_binding` (16 past it), its import settings' wrap and filter (#1323). This is the layout the forward pass's
/// group(2) is built against (#202, #207).
pub(crate) fn create_material_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Material Layout"),
        entries: &material_entries(),
    })
}

/// [`create_material_layout`]'s entries.
pub(crate) fn material_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    use rusty_shadergen::textures::sampler_binding;
    let params = wgpu::BindGroupLayoutEntry {
        binding: rusty_shadergen::params::PARAM_BINDING,
        // The vertex stage too: a vertex block's runtime param (#1524).
        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: wgpu::BufferSize::new(PARAMS_SIZE),
        },
        count: None,
    };
    let mut entries = vec![material_sampler_entry(1), params];
    for binding in all_bindings() {
        entries.push(material_texture_entry(binding));
        // Albedo's sampler is `s_diffuse`, at 1, pushed above.
        if binding != 0 {
            entries.push(material_sampler_entry(sampler_binding(binding)));
        }
    }
    entries
}

/// Main shadow bind group layout: the cascade uniform, the cascade depth array, the
/// comparison sampler (#435), the view's ambient-occlusion texture (#436), and the
/// point/spot shadow atlas with its tiles (#468), and the opaque copy a refracting
/// or absorbing surface reads (#1523: colour and eye distance, and its sampler) — the
/// forward pass's scene group, everything it reads that a per-frame pass produced.
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

/// The forward pass's scene group over [`create_shadow_layout`]: the cascade `uniform`,
/// the shadow renderer's active cascade array + sampler, `ao` — a view's SSAO
/// result, or the 1x1 white fallback where no AO ran — and `opaque`, the view's
/// opaque copy in its transparent pass or the 1x1 fallback everywhere else (#1523).
pub(crate) fn create_shadow_bind_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    uniform: &wgpu::Buffer,
    shadows: &crate::passes::shadows::ShadowRenderer,
    ao: &wgpu::TextureView,
    opaque: OpaqueViews,
) -> wgpu::BindGroup {
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
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
