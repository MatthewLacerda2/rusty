
#import fullscreen
#import lighting::ssr_surface
#import lighting::ssr_march
#import numeric

struct SsrParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    march: vec4<f32>,
    surface: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: SsrParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var t_weight: texture_2d<f32>;
@group(0) @binding(5) var t_applied: texture_2d<f32>;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / world.w;
}

fn view_forward() -> vec3<f32> {
    let near = world_from_depth(vec2<f32>(0.5), 0.0);
    return normalize(world_from_depth(vec2<f32>(0.5), 0.5) - near);
}

fn world_at(uv: vec2<f32>, depth: f32, fwd: vec3<f32>) -> vec3<f32> {
    let a = world_from_depth(uv, 0.0);
    let b = world_from_depth(uv, 0.5);
    let da = dot(fwd, a - params.camera_pos.xyz);
    let db = dot(fwd, b - params.camera_pos.xyz);
    return a + (b - a) * ((depth - da) / (db - da));
}

const NEAR_MIRROR: f32 = 0.05;

// The tangent of the half-angle of the cone a surface of perceptual `roughness` reflects into,
// Uludag's (GPU Pro 5, *Hi-Z Screen-Space Cone-Traced Reflections*): GGX's `alpha = roughness²`
// as a Blinn-Phong power `2 / alpha² − 2`, and the cone that holds its lobe down to 0.244 of
// its peak, held to 0 up to `NEAR_MIRROR`.
fn cone(roughness: f32) -> f32 {
    let alpha = max(roughness * roughness, 1e-3);
    let power = 2.0 / (alpha * alpha) - 2.0;
    let c = pow(0.244, 1.0 / (power + 1.0));
    let sharp = clamp((roughness - NEAR_MIRROR) / NEAR_MIRROR, 0.0, 1.0);
    return sqrt(max(1.0 - c * c, 0.0)) / c * sharp;
}

fn world_px(p: vec2<i32>, fwd: vec3<f32>) -> vec3<f32> {
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(textureDimensions(t_velocity));
    return world_at(uv, textureLoad(t_velocity, p, 0).z, fwd);
}

fn near(px: vec2<i32>, o: vec2<i32>, fwd: vec3<f32>) -> vec3<f32> {
    let top = vec2<i32>(textureDimensions(t_velocity)) - 1;
    return world_px(clamp(px + o, vec2<i32>(0), top), fwd);
}

fn geometric_normal(px: vec2<i32>, world: vec3<f32>, fwd: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var xs = array<vec3<f32>, 2>(near(px, vec2<i32>(1, 0), fwd) - world, world - near(px, vec2<i32>(-1, 0), fwd));
    var ys = array<vec3<f32>, 2>(near(px, vec2<i32>(0, 1), fwd) - world, world - near(px, vec2<i32>(0, -1), fwd));
    let eye = world - params.camera_pos.xyz;
    var best = vec3<f32>(0.0);
    var best_dot = -2.0;
    for (var i = 0; i < 4; i = i + 1) {
        let c = cross(ys[i >> 1u], xs[i & 1]);
        if (dot(c, c) < 1e-20) {
            continue;
        }
        let g = faceForward(normalize(c), eye, c);
        if (dot(g, n) > best_dot) {
            best = g;
            best_dot = dot(g, n);
        }
    }
    return best;
}

fn spread(world: vec3<f32>, t: f32, c: f32, dim: vec2<f32>) -> f32 {
    return min(across(world, t, c, dim), ssr_march::BLUR_FADE_END * min(dim.x, dim.y));
}

fn across(world: vec3<f32>, t: f32, c: f32, dim: vec2<f32>) -> f32 {
    let eye = params.camera_pos.xyz;
    let d0 = distance(world, eye);
    var side = cross(world - eye, vec3<f32>(0.0, 1.0, 0.0));
    if (dot(side, side) < 1e-10) {
        side = cross(world - eye, vec3<f32>(1.0, 0.0, 0.0));
    }
    let f = ssr_march::Frame(params.view_proj, eye, vec3<f32>(0.0), vec4<f32>(0.0));
    let a = ssr_march::project(f, world).xy;
    let b = ssr_march::project(f, world + normalize(side) * (c * t * d0 / max(d0 + t, 1e-4))).xy;
    return length((b - a) * dim);
}

// The cone a hit's radius is measured with for the resolve's field: the trace stores [`spread`]
// per unit of cone, measured at a typical satin cone (about roughness 0.4's), where the
// projection is close to linear, so each pixel scales it by its own cone.
const UNIT_CONE: f32 = 0.2;
const UNIT_MAX: f32 = 16384.0;

// What a hit's count is stored times, so the count of one hit in a coarse level of the spread
// pyramid (a 4^-8 share of it at level 8) stays a normal f16, not a subnormal a GPU may flush
// to 0 as it writes the level: the mean radius of a few hits in a wide window then holds, not
// just a crowd's.
const COUNT_SCALE: f32 = 64.0;

const FOUND_FLOOR: f32 = 0.4;

struct Traced {
    @location(0) hit: vec4<f32>,
    @location(1) spread: vec4<f32>,
};

// Whether pixel `px` takes SSR at all: its surface reflects, is smooth enough (`fade` above 0)
// and is not the sky.
fn takes(s: ssr_surface::SsrSurface, fade: f32, depth: f32) -> bool {
    return ssr_surface::reflects(s) && fade > 0.0 && depth < ssr_march::SKY_DEPTH;
}

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
    if (dot(r, geometric_normal(px, world, fwd, s.normal)) < 0.0) {
        return missed;
    }
    let origin = world + s.normal * 0.02;
    let f = ssr_march::Frame(params.view_proj, params.camera_pos.xyz, fwd, vec4<f32>(0.0, 0.0, 1.0, 0.0));
    let c = cone(s.roughness);
    let hit = ssr_march::trace_hit(f, t_velocity, t_color, s_color, origin, r, i32(params.march.x), params.march.y, c, 0.0);
    if (hit.dist <= 0.0) {
        return missed;
    }
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

const TAPS: i32 = 12;
const GOLDEN_ANGLE: f32 = 2.3999632;
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

fn mean_unit(uv: vec2<f32>, lod: f32, fallback: f32, trust: f32) -> f32 {
    let s = textureSampleLevel(t_spread, s_color, uv, lod);
    let count = s.z / COUNT_SCALE + trust;
    return (s.y + trust * fallback) / max(count, LONE * exp2(-2.0 * lod));
}

const LONE: f32 = 0.02;

const RADIUS_TRUST: f32 = 0.02;
const REFINE: i32 = 2;

fn blur_radius(uv: vec2<f32>, world: vec3<f32>, roughness: f32, dim: vec2<f32>) -> f32 {
    let c = cone(roughness);
    let cap = spread(world, params.march.x * params.march.y, c, dim);
    if (cap < SHARP) {
        return 0.0;
    }
    var unit = mean_unit(uv, log2(cap), 0.0, 0.0);
    for (var i = 0; i < REFINE; i = i + 1) {
        unit = mean_unit(uv, log2(max(min(c * unit, cap), 1.0)), unit, RADIUS_TRUST);
    }
    return min(c * unit, cap);
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
        let lod = max(log2(radius * 0.5), 0.0);
        var sum = vec4<f32>(0.0);
        var traced = 0.0;
        for (var i = 0; i < TAPS; i = i + 1) {
            let a = f32(i) * GOLDEN_ANGLE;
            let q = uv + vec2<f32>(cos(a), sin(a)) * radius * sqrt((f32(i) + 0.5) / f32(TAPS)) / dim;
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
