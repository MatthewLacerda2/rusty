#import fullscreen
#import lighting::ssr_surface
#import lighting::ssr_march
#import lighting::ssr_vogel
#import numeric
#import postfx::ssr_view::{SsrParams, params, t_color, s_color, t_velocity, t_weight, t_applied, world_from_depth, view_forward, world_at, NEAR_MIRROR, cone, world_px, near, geometric_normal, spread, across, UNIT_CONE, UNIT_MAX, COUNT_SCALE, FOUND_FLOOR, takes}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

struct Traced {
    @location(0) hit: vec4<f32>,
    @location(1) spread: vec4<f32>,
};

@fragment
fn fs_trace(@builtin(position) pos: vec4<f32>) -> Traced {
    let px = vec2<i32>(pos.xy);
    let dim = vec2<f32>(textureDimensions(t_color));
    let uv = pos.xy / dim;
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let v = textureLoad(t_velocity, px, 0);
    let fade = ssr_surface::smoothness_fade(s.roughness, params.surface.x, params.surface.y);
    if (!takes(s, fade, v.z)) {
        discard;
    }
    let fwd = view_forward();
    let world = world_at(uv, v.z, fwd);
    let r = reflect(normalize(world - params.camera_pos.xyz), s.normal);
    let missed = Traced(vec4<f32>(0.0), vec4<f32>(1.0, 0.0, 0.0, 0.0));
    // A normal map can aim the ray into its own surface: a miss, or the floor reflects itself.
    if (dot(r, geometric_normal(px, world, fwd, s.normal)) < 0.0) {
        return missed;
    }
    let origin = world + s.normal * 0.02;
    let f = ssr_march::Frame(params.view_proj, params.camera_pos.xyz, fwd, vec4<f32>(0.0, 0.0, 1.0, 0.0));
    let c = cone(s.roughness);
    // Sharp (`blur_scale` 0): the resolve blurs hits by the cone.
    let hit = ssr_march::trace_hit(f, t_velocity, t_color, s_color, origin, r, i32(params.march.x), params.march.y, c, 0.0);
    if (hit.dist <= 0.0) {
        return missed;
    }
    // An emissive past f16 is `inf` in the copy: no reflection.
    let w = select(0.0, hit.colour.w, numeric::finite4(hit.colour));
    let unit = min(across(world, hit.dist, UNIT_CONE, dim) / UNIT_CONE, UNIT_MAX);
    let k = max(w, FOUND_FLOOR);
    let found = vec4<f32>(1.0, unit * k, k * COUNT_SCALE, 0.0);
    if (w <= 0.0) {
        return Traced(vec4<f32>(0.0), found);
    }
    return Traced(vec4<f32>(hit.colour.rgb * w, w), found);
}

@group(1) @binding(0) var t_hits: texture_2d<f32>;
@group(1) @binding(1) var t_spread: texture_2d<f32>;

// Pixels; below it a hit reads where traced.
const SHARP: f32 = 0.5;

fn on_plane(q: vec2<f32>, world: vec3<f32>, g: vec3<f32>, fwd: vec3<f32>) -> bool {
    let dim = vec2<i32>(textureDimensions(t_velocity));
    let p = clamp(vec2<i32>(q * vec2<f32>(dim)), vec2<i32>(0), dim - 1);
    let depth = textureLoad(t_velocity, p, 0).z;
    if (depth >= ssr_march::SKY_DEPTH) {
        return false;
    }
    let there = world_px(p, fwd);
    return abs(dot(g, there - world)) <= max(ssr_march::THICKNESS, depth * ssr_march::DEPTH_SLACK);
}

fn mean_unit(uv: vec2<f32>, lod: f32, fallback: f32, prior: f32) -> f32 {
    let s = textureSampleLevel(t_spread, s_color, uv, lod);
    let count = s.z / COUNT_SCALE + prior;
    return (s.y + prior * fallback) / max(count, LONE * exp2(-2.0 * lod));
}

// Share of one hit's pixel below which a window holds nothing.
const LONE: f32 = 0.02;

const WIDER_WINDOW_PRIOR_HITS: f32 = 0.02;
const NARROWING_PASSES: i32 = 2;

// The hits' mean radius, not this ray's own; plain on purpose: a nearest-biased mean
// collapsed short rays' radii into blotches.
fn blur_radius(uv: vec2<f32>, world: vec3<f32>, roughness: f32, dim: vec2<f32>) -> f32 {
    let c = cone(roughness);
    let cap = spread(world, params.march.x * params.march.y, c, dim);
    if (cap < SHARP) {
        return 0.0;
    }
    var unit = mean_unit(uv, log2(cap), 0.0, 0.0);
    for (var i = 0; i < NARROWING_PASSES; i = i + 1) {
        unit = mean_unit(uv, log2(max(min(c * unit, cap), 1.0)), unit, WIDER_WINDOW_PRIOR_HITS);
    }
    return min(c * unit, cap);
}

fn level_as_wide_as_tap_gap(radius: f32) -> f32 {
    return max(log2(radius * ssr_vogel::TAP_GAP_PER_RADIUS), 0.0);
}

@fragment
fn fs_resolve(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let dim = vec2<f32>(textureDimensions(t_hits));
    let uv = pos.xy / dim;
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let v = textureLoad(t_velocity, px, 0);
    let fade = ssr_surface::smoothness_fade(s.roughness, params.surface.x, params.surface.y);
    if (!takes(s, fade, v.z)) {
        discard;
    }
    let fwd = view_forward();
    let world = world_at(uv, v.z, fwd);
    var p = textureLoad(t_hits, px, 0);
    let radius = blur_radius(uv, world, s.roughness, dim);
    if (radius >= SHARP) {
        var g = geometric_normal(px, world, fwd, s.normal);
        g = select(s.normal, g, dot(g, g) > 0.0);
        let lod = level_as_wide_as_tap_gap(radius);
        var sum = vec4<f32>(0.0);
        var traced = 0.0;
        for (var i = 0; i < ssr_vogel::TAPS; i = i + 1) {
            let a = f32(i) * ssr_vogel::GOLDEN_ANGLE_RADIANS;
            let q = uv + vec2<f32>(cos(a), sin(a)) * radius * sqrt((f32(i) + 0.5) / f32(ssr_vogel::TAPS)) / dim;
            if (any(q < vec2<f32>(0.0)) || any(q > vec2<f32>(1.0)) || !on_plane(q, world, g, fwd)) {
                continue;
            }
            sum = sum + textureSampleLevel(t_hits, s_color, q, lod);
            traced = traced + textureSampleLevel(t_spread, s_color, q, lod).x;
        }
        p = sum / max(traced, 1e-4);
    }
    if (p.a * fade <= 0.0) {
        discard;
    }
    let c = textureLoad(t_color, px, 0);
    return vec4<f32>(max(c.rgb + (s.weight * p.rgb - s.applied * p.a) * fade, vec3<f32>(0.0)), c.a);
}
