// SSR's frame: its uniform, its group-0 inputs and the view helpers every SSR pass reads
// (`postfx/ssr.wgsl`'s march and resolve, `postfx/ssr_rays.wgsl`'s rays).

#define_import_path postfx::ssr_view

#import lighting::ssr_surface
#import lighting::ssr_march
#import common::Fog

struct SsrParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
    march: vec4<f32>,
    surface: vec4<f32>,
    // *Mixed*: x the smoothness a ray needs, y its reach (world units), z the trace's
    // resolution divisor, w 1 when rays trace.
    rays: vec4<f32>,
    // The scene's fog, which a ray hit takes as a march hit does.
    fog: Fog,
};

@group(0) @binding(0) var<uniform> params: SsrParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var t_weight: texture_2d<f32>;
@group(0) @binding(5) var t_applied: texture_2d<f32>;

fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / world.w;
}

fn view_forward() -> vec3<f32> {
    let near = world_from_depth(vec2<f32>(0.5), 0.0);
    return normalize(world_from_depth(vec2<f32>(0.5), 0.5) - near);
}

// View depth is affine along a pixel's ray.
fn world_at(uv: vec2<f32>, depth: f32, fwd: vec3<f32>) -> vec3<f32> {
    let a = world_from_depth(uv, 0.0);
    let b = world_from_depth(uv, 0.5);
    let da = dot(fwd, a - params.camera_pos.xyz);
    let db = dot(fwd, b - params.camera_pos.xyz);
    return a + (b - a) * ((depth - da) / (db - da));
}

// Up to this roughness a hit reads sharp: its lobe's blur would grow with resolution.
const NEAR_MIRROR: f32 = 0.05;

// Tangent of the lobe's half-angle (Uludag, GPU Pro 5): GGX as Blinn-Phong, to 0.244 of its peak.
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

// The nearest `n` of four one-sided normals: at an edge, one crosses onto another surface.
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

// [`spread`] uncapped.
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

// [`spread`] is stored per unit of this cone (roughness 0.4's, near linear); each pixel
// scales by its own.
const UNIT_CONE: f32 = 0.2;
// Keeps radius × count finite in f16.
const UNIT_MAX: f32 = 16384.0;

// Counts are stored times this: one hit at level 8 (a 4^-8 share) stays a normal f16, not
// flushed to 0.
const COUNT_SCALE: f32 = 64.0;

// A faded found hit's count (1 at full weight); lower, the mean radius swings at a fading
// edge.
const FOUND_FLOOR: f32 = 0.4;

fn takes(s: ssr_surface::SsrSurface, fade: f32, depth: f32) -> bool {
    return ssr_surface::reflects(s) && fade > 0.0 && depth < ssr_march::SKY_DEPTH;
}
