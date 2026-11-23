// SSR's *Mixed* tracing: `fs_ray` traces the march's misses against the static scene and
// shades the hits as diffuse; `fs_merge` folds them into the march's hits.

#import fullscreen
#import common::{LightingUniforms, PI}
#import lighting::ambient::ambient_radiance
#import lighting::sh::sh_basis
#import lighting::ssr_surface
#import forward::bindings::{MaterialData, ObjectData}
#import rt::trace::{Hit, trace_closest, trace_any, hit_found, hit_uvs, hit_record, hit_normal}
#import postfx::ssr_view::{params, t_velocity, t_weight, t_applied, view_forward, world_at, geometric_normal, across, takes, UNIT_CONE, UNIT_MAX, COUNT_SCALE, FOUND_FLOOR}

@group(1) @binding(0) var t_march_hits: texture_2d<f32>;
@group(1) @binding(1) var t_march_spread: texture_2d<f32>;

@group(2) @binding(0) var<uniform> rays_lighting: LightingUniforms;
@group(2) @binding(1) var<storage, read> rays_objects: array<ObjectData>;
@group(2) @binding(2) var<storage, read> rays_materials: array<MaterialData>;
@group(2) @binding(3) var t_rays_lightmaps: texture_2d_array<f32>;
@group(2) @binding(4) var t_rays_masks: texture_2d_array<f32>;
@group(2) @binding(5) var s_rays_clamp: sampler;
@group(2) @binding(6) var t_rays_albedo: texture_2d_array<f32>;
// By material record.
@group(2) @binding(7) var<storage, read> rays_albedo_layers: array<u32>;
@group(2) @binding(8) var s_rays_repeat: sampler;
// rgb radiance, a distance (0 none).
@group(2) @binding(9) var t_rays: texture_2d<f32>;

const NO_LAYER: u32 = 0xffffffffu;
const RAY_OFFSET: f32 = 0.02;
// A ray's last fifth fades, as a march's does.
const REACH_FADE: f32 = 0.2;
const SHADOW_REACH: f32 = 10000.0;
// A shared ray counts on the same surface only: depth share, normal cosine.
const SAME_DEPTH: f32 = 0.05;
const SAME_NORMAL: f32 = 0.9;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn divisor() -> i32 {
    return max(i32(params.rays.z), 1);
}

// A ray texel's block's centre.
fn source_of(q: vec2<i32>) -> vec2<i32> {
    let d = divisor();
    let top = vec2<i32>(textureDimensions(t_velocity)) - 1;
    return clamp(q * d + d / 2, vec2<i32>(0), top);
}

struct Pixel {
    surface: ssr_surface::SsrSurface,
    depth: f32,
    traces: bool,
};

fn pixel(px: vec2<i32>) -> Pixel {
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let depth = textureLoad(t_velocity, px, 0).z;
    let fade = ssr_surface::smoothness_fade(s.roughness, params.surface.x, params.surface.y);
    let smooth_enough = 1.0 - s.roughness >= params.rays.x;
    return Pixel(s, depth, takes(s, fade, depth) && smooth_enough);
}

fn eval_sh(n: vec3<f32>, o: ObjectData) -> vec3<f32> {
    let b = sh_basis(n);
    var lobe = array<f32, 9>(PI, 2.0 * PI / 3.0, 2.0 * PI / 3.0, 2.0 * PI / 3.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0, PI / 4.0);
    var out = vec3<f32>(0.0);
    for (var i = 0u; i < 9u; i = i + 1u) {
        out += o.sh[i].xyz * b[i] * lobe[i];
    }
    return max(out, vec3<f32>(0.0));
}

fn albedo_layer(record: u32) -> u32 {
    if (record >= arrayLength(&rays_albedo_layers)) {
        return NO_LAYER;
    }
    return rays_albedo_layers[record];
}

// The hit's footprint over the map, a pixel cone from `t0 + t` metres.
fn albedo_lod(t: f32, t0: f32, st: vec2<f32>) -> f32 {
    let size = f32(textureDimensions(t_rays_albedo).x);
    let pixel = 2.0 * (t0 + t) / f32(textureDimensions(t_velocity).y);
    let top = f32(textureNumLevels(t_rays_albedo) - 1u);
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

// Diffuse: no specular at the hit.
fn shade(hit: Hit, origin: vec3<f32>, d: vec3<f32>, t0: f32) -> vec3<f32> {
    let record = hit_record(hit);
    let m = rays_materials[record];
    let o = rays_objects[hit.slot];
    let uvs = hit_uvs(hit);
    let n = hit_normal(hit, d);
    let p = origin + d * hit.t;
    var albedo = m.color_tint.rgb;
    let layer = albedo_layer(record);
    if (layer != NO_LAYER && m.use_texture == 1u) {
        let uv = uvs.xy * m.uv_st.xy + m.uv_st.zw;
        let lod = albedo_lod(hit.t, t0, m.uv_st.xy);
        albedo *= textureSampleLevel(t_rays_albedo, s_rays_repeat, uv, layer, lod).rgb;
    }
    let diffuse = albedo * (1.0 - m.metallic);
    return diffuse * irradiance(o, uvs, p, n) + m.emissive.rgb;
}

@fragment
fn fs_ray(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = source_of(vec2<i32>(pos.xy));
    let here = pixel(px);
    // A march hit at full confidence is never re-traced.
    if (!here.traces || textureLoad(t_march_hits, px, 0).a >= 1.0) {
        return vec4<f32>(0.0);
    }
    let dim = vec2<f32>(textureDimensions(t_velocity));
    let uv = (vec2<f32>(px) + 0.5) / dim;
    let fwd = view_forward();
    let world = world_at(uv, here.depth, fwd);
    let eye = params.camera_pos.xyz;
    let r = reflect(normalize(world - eye), here.surface.normal);
    if (dot(r, geometric_normal(px, world, fwd, here.surface.normal)) < 0.0) {
        return vec4<f32>(0.0);
    }
    let origin = world + here.surface.normal * RAY_OFFSET;
    let hit = trace_closest(origin, r, params.rays.y);
    if (!hit_found(hit)) {
        return vec4<f32>(0.0);
    }
    let c = shade(hit, origin, r, distance(world, eye));
    // Past f16, `inf`: no reflection, as the march's.
    return select(vec4<f32>(0.0), vec4<f32>(c, hit.t), all(c < vec3<f32>(65000.0)));
}

struct Merged {
    @location(0) hit: vec4<f32>,
    @location(1) spread: vec4<f32>,
};

// `px`'s ray texels, bilinear, on its surface only: rgb radiance, a the share that hit.
struct Shared {
    colour: vec4<f32>,
    dist: f32,
};

fn shared_ray(px: vec2<i32>, here: Pixel) -> Shared {
    let d = divisor();
    let rays = vec2<i32>(textureDimensions(t_rays));
    let at = (vec2<f32>(px) + 0.5) / f32(d) - 0.5;
    let base = vec2<i32>(floor(at));
    let f = at - floor(at);
    var sum = vec3<f32>(0.0);
    var dist = 0.0;
    var hits = 0.0;
    var weight = 0.0;
    for (var i = 0; i < 4; i = i + 1) {
        let o = vec2<i32>(i & 1, i >> 1);
        let q = clamp(base + o, vec2<i32>(0), rays - 1);
        let src = pixel(source_of(q));
        let near_depth = abs(src.depth - here.depth) <= SAME_DEPTH * here.depth;
        let same = near_depth && dot(src.surface.normal, here.surface.normal) >= SAME_NORMAL;
        let w = select(1.0 - f.x, f.x, o.x == 1) * select(1.0 - f.y, f.y, o.y == 1);
        if (!same || w <= 0.0) {
            continue;
        }
        let ray = textureLoad(t_rays, q, 0);
        weight += w;
        if (ray.a > 0.0) {
            sum += ray.rgb * w;
            dist += ray.a * w;
            hits += w;
        }
    }
    if (hits <= 0.0) {
        return Shared(vec4<f32>(0.0), 0.0);
    }
    return Shared(vec4<f32>(sum / hits, hits / weight), dist / hits);
}

@fragment
fn fs_merge(@builtin(position) pos: vec4<f32>) -> Merged {
    let px = vec2<i32>(pos.xy);
    let march = Merged(textureLoad(t_march_hits, px, 0), textureLoad(t_march_spread, px, 0));
    let here = pixel(px);
    let w = march.hit.a;
    if (!here.traces || w >= 1.0) {
        return march;
    }
    let ray = shared_ray(px, here);
    let reach = params.rays.y;
    let wr = ray.colour.a * clamp((reach - ray.dist) / (reach * REACH_FADE), 0.0, 1.0);
    if (wr <= 0.0) {
        return march;
    }
    let dim = vec2<f32>(textureDimensions(t_velocity));
    let fwd = view_forward();
    let world = world_at((vec2<f32>(px) + 0.5) / dim, here.depth, fwd);
    let share = (1.0 - w) * wr;
    let total = w + share;
    let march_unit = march.spread.y / max(march.spread.z / COUNT_SCALE, 1e-4);
    let ray_unit = min(across(world, ray.dist, UNIT_CONE, dim) / UNIT_CONE, UNIT_MAX);
    let unit = (march_unit * w + ray_unit * share) / total;
    let k = max(total, FOUND_FLOOR);
    let hit = vec4<f32>(march.hit.rgb + ray.colour.rgb * share, total);
    return Merged(hit, vec4<f32>(1.0, unit * k, k * COUNT_SCALE, 0.0));
}

// `fs_resample`'s: one base colour map, read down to a layer of `t_rays_albedo`.
@group(0) @binding(10) var t_source: texture_2d<f32>;
@group(0) @binding(11) var s_source: sampler;

@fragment
fn fs_resample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(t_source));
    let lod = log2(max(max(size.x, size.y) / 256.0, 1.0));
    return textureSampleLevel(t_source, s_source, pos.xy / 256.0, lod);
}
