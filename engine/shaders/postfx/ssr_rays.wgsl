// SSR's *Mixed* tracing: `fs_ray` traces the march's misses against the static scene and
// shades the hits as diffuse; `fs_merge` folds them into the march's hits.

#import fullscreen
#import common::fog_factor
#import lighting::ssr_surface
#import rt::trace::{trace_closest, hit_found}
#import postfx::ssr_shade::{shade, RAY_OFFSET, REACH_FADE}
#import postfx::ssr_view::{params, t_velocity, t_weight, t_applied, view_forward, world_at, geometric_normal, across, takes, UNIT_CONE, UNIT_MAX, COUNT_SCALE, FOUND_FLOOR}

@group(1) @binding(0) var t_march_hits: texture_2d<f32>;
@group(1) @binding(1) var t_march_spread: texture_2d<f32>;

// rgb radiance, a distance (0 none).
@group(2) @binding(9) var t_rays: texture_2d<f32>;

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
    let t0 = distance(world, eye);
    // Fogged as the opaque image fogs a march hit, over the line from the eye to the hit,
    // so the two meet without a seam. The volumetric fog's grid holds only the view's frustum.
    let fog = fog_factor(params.fog, origin + r * hit.t, eye);
    let c = mix(shade(hit, origin, r, t0), params.fog.color, fog);
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

// `fs_resample`'s: one map, read down to a layer of `t_rays_maps`.
@group(0) @binding(10) var t_source: texture_2d<f32>;
@group(0) @binding(11) var s_source: sampler;

@fragment
fn fs_resample(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let size = vec2<f32>(textureDimensions(t_source));
    let lod = log2(max(max(size.x, size.y) / 256.0, 1.0));
    return textureSampleLevel(t_source, s_source, pos.xy / 256.0, lod);
}
