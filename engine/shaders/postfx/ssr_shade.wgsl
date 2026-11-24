// SSR's *Mixed* ray hit, shaded as diffuse: what `postfx/ssr_rays.wgsl`'s opaque rays and
// `postfx/ssr_transparent.wgsl`'s translucent ones share (their group 2, then `rt::trace`'s).

#define_import_path postfx::ssr_shade

#import common::{LightingUniforms, PI}
#import lighting::ambient::ambient_radiance
#import lighting::sh::sh_basis
#import forward::bindings::{MaterialData, ObjectData}
#import forward::maps::map_factor
#import rt::trace::{Hit, trace_any, hit_uvs, hit_record, hit_normal}
#import postfx::ssr_view::t_velocity

@group(2) @binding(0) var<uniform> rays_lighting: LightingUniforms;
@group(2) @binding(1) var<storage, read> rays_objects: array<ObjectData>;
@group(2) @binding(2) var<storage, read> rays_materials: array<MaterialData>;
@group(2) @binding(3) var t_rays_lightmaps: texture_2d_array<f32>;
@group(2) @binding(4) var t_rays_masks: texture_2d_array<f32>;
@group(2) @binding(5) var s_rays_clamp: sampler;
// `rays::maps`.
@group(2) @binding(6) var t_rays_maps: texture_2d_array<f32>;
// By material record: base colour layer, metallic layer.
@group(2) @binding(7) var<storage, read> rays_map_layers: array<vec2<u32>>;
@group(2) @binding(8) var s_rays_repeat: sampler;

const NO_LAYER: u32 = 0xffffffffu;
const RAY_OFFSET: f32 = 0.02;
// A ray's last fifth fades, as a march's does.
const REACH_FADE: f32 = 0.2;
const SHADOW_REACH: f32 = 10000.0;

fn eval_sh(n: vec3<f32>, o: ObjectData) -> vec3<f32> {
    let b = sh_basis(n);
    var lobe = array<f32, 9>(PI, 2.0 * PI / 3.0, 2.0 * PI / 3.0, 2.0 * PI / 3.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0);
    var out = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        out += o.sh[i].xyz * b[i] * lobe[i];
    }
    return max(out, vec3<f32>(0.0));
}

fn map_layers(record: u32) -> vec2<u32> {
    if (record >= arrayLength(&rays_map_layers)) {
        return vec2<u32>(NO_LAYER);
    }
    return rays_map_layers[record];
}

// The hit's footprint over the map, a pixel cone from `t0 + t` metres.
fn map_lod(t: f32, t0: f32, st: vec2<f32>) -> f32 {
    let size = f32(textureDimensions(t_rays_maps).x);
    let pixel = 2.0 * (t0 + t) / f32(textureDimensions(t_velocity).y);
    let top = f32(textureNumLevels(t_rays_maps) - 1u);
    return clamp(log2(max(pixel * max(st.x, st.y) * size, 1.0)), 0.0, top);
}

// E / pi, as the forward pass picks it, plus each directional light the page lacks, shadowed
// by a ray or the shadowmask.
fn irradiance(o: ObjectData, uvs: vec4<f32>, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var e: vec3<f32>;
    let luv = uvs.zw * o.lightmap_st.xy + o.lightmap_st.zw;
    let paged = o.lightmap_page > 0u;
    if (paged) {
        e = textureSampleLevel(t_rays_lightmaps, s_rays_clamp, luv, o.lightmap_page - 1u, 0.0).rgb;
    } else if (o.use_sh == 1u) {
        e = eval_sh(n, o) / PI;
    } else {
        e = ambient_radiance(rays_lighting.ambient, n);
    }
    var mask = vec4<f32>(1.0);
    let masked = rays_lighting.shadowmask > 0.5 && paged && rays_lighting.masks_bound > 0.5;
    if (masked) {
        mask = textureSampleLevel(t_rays_masks, s_rays_clamp, luv, o.lightmap_page - 1u, 0.0);
    }
    for (var i = 0u; i < rays_lighting.num_dir_lights; i = i + 1u) {
        let sun = rays_lighting.dir_lights[i];
        if (paged && sun.baked > 0.5) {
            continue;
        }
        let L = normalize(-sun.direction);
        let ndl = dot(n, L);
        if (ndl <= 0.0) {
            continue;
        }
        var lit = 1.0;
        if (masked && sun.mask_channel >= 0.0) {
            lit = mask[u32(clamp(sun.mask_channel, 0.0, 3.0))];
        } else if (trace_any(p + n * RAY_OFFSET, L, SHADOW_REACH)) {
            lit = 0.0;
        }
        e += sun.color * sun.intensity * ndl * lit / PI;
    }
    return e;
}

// Diffuse: no specular at the hit; the forward pass's metallic.
fn shade(hit: Hit, origin: vec3<f32>, d: vec3<f32>, t0: f32) -> vec3<f32> {
    let record = hit_record(hit);
    let m = rays_materials[record];
    let o = rays_objects[hit.slot];
    let uvs = hit_uvs(hit);
    let n = hit_normal(hit, d);
    let p = origin + d * hit.t;
    let layers = map_layers(record);
    let uv = uvs.xy * m.uv_st.xy + m.uv_st.zw;
    let lod = map_lod(hit.t, t0, m.uv_st.xy);
    var albedo = m.color_tint.rgb;
    if (layers.x != NO_LAYER && m.use_texture == 1u) {
        albedo *= textureSampleLevel(t_rays_maps, s_rays_repeat, uv, layers.x, lod).rgb;
    }
    var mask = 1.0;
    if (layers.y != NO_LAYER) {
        let texel = textureSampleLevel(t_rays_maps, s_rays_repeat, uv, layers.y, lod);
        mask = map_factor(texel, m.use_metallic_map, 2u);
    }
    let diffuse = albedo * (1.0 - clamp(m.metallic * mask, 0.0, 1.0));
    return diffuse * irradiance(o, uvs, p, n) + m.emissive.rgb;
}
