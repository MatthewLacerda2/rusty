// engine/shaders/postfx/ssr.wgsl — screen-space reflections (#1521, #1526): one
// fullscreen pass over the base camera's opaque image, run right after its opaque
// pass (`render::postfx::effects::ssr::pass`), before the translucents, the overlay
// cameras and every post effect. High tier only; on every tier the forward pass
// reflects the sky or a reflection probe (#718).
//
// It reads three targets of the view's own: the velocity target (#1435) for each
// pixel's depth along the view axis (`.z`; the sky at `SKY_DEPTH`), and the two SSR
// surface targets (`lighting/ssr_surface.wgsl`) for its shading normal, the weight a
// hit gets and the environment reflection the forward pass applied. So only a smooth
// surface reflects.
//
// An SSR hit **replaces** the environment reflection the forward pass applied, sky or
// reflection probe alike, as HDRP's reflection hierarchy does: `pixel − applied +
// weight × hit`, both already through the pixel's `ao` and fog. A miss leaves the
// pixel as the forward pass lit it, so SSR never samples the sky itself (#1542).
//
// Running here, not in the post chain, is HDRP's order: the translucents, particles
// and world canvases blend over the reflected image, so their coverage scales the
// reflection like any other light under them; depth of field and motion blur blur
// it; an overlay camera (a viewmodel) has not drawn yet, so it gets none and hides
// nothing from the march. A pixel that gets no SSR is discarded: the pass loads the
// image and leaves it as it was.
//
// No derivatives here (src/render/CLAUDE.md, *Shaders*): every read is a
// `textureLoad` or an explicit-LOD sample, so the per-pixel branches below are safe.

#import fullscreen
#import lighting::ssr_surface

struct SsrParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // xyz: the base camera's eye
    camera_pos: vec4<f32>,
    // x: the march's step count, y: its step length in world units, from the
    // volume's quality (`render::postfx::effects::ssr::march`)
    march: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: SsrParams;
// A copy of the opaque image, which the pass draws over.
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;
@group(0) @binding(3) var t_velocity: texture_2d<f32>;
@group(0) @binding(4) var t_weight: texture_2d<f32>;
@group(0) @binding(5) var t_applied: texture_2d<f32>;

// How far from a screen edge, as a fraction of the screen, a hit fades out (HDRP's
// *Screen Edge Fade Distance*).
const SCREEN_FADE: f32 = 0.1;
// Bisection steps that refine a hit between the last two march points.
const REFINE: i32 = 4;
// A velocity texel's depth at or beyond this is the sky (`render::motion::SKY_DISTANCE`).
const SKY_DEPTH: f32 = 60000.0;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> @builtin(position) vec4<f32> {
    return fullscreen::triangle(vid);
}

fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = params.inv_view_proj * ndc;
    return world.xyz / world.w;
}

// The base camera's view axis, from its inverse view-projection.
fn view_forward() -> vec3<f32> {
    let near = world_from_depth(vec2<f32>(0.5), 0.0);
    return normalize(world_from_depth(vec2<f32>(0.5), 0.5) - near);
}

// The point at `uv` whose depth along the view axis `fwd` is `depth`, for a
// perspective or orthographic camera alike: depth is affine along the pixel's ray.
fn world_at(uv: vec2<f32>, depth: f32, fwd: vec3<f32>) -> vec3<f32> {
    let a = world_from_depth(uv, 0.0);
    let b = world_from_depth(uv, 0.5);
    let da = dot(fwd, a - params.camera_pos.xyz);
    let db = dot(fwd, b - params.camera_pos.xyz);
    return a + (b - a) * ((depth - da) / (db - da));
}

fn texel(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let dim = vec2<f32>(textureDimensions(t));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * dim);
    return textureLoad(t, coord, 0);
}

// Where a march point `p` lies against the screen: `on_screen` false when it left
// the screen or went behind the camera; else its `uv`, and `behind`, how far it is
// behind the surface there (> 0: behind), which counts only where `valid` (not the
// sky).
struct Probe {
    behind: f32,
    uv: vec2<f32>,
    valid: bool,
    on_screen: bool,
};

fn probe(p: vec3<f32>, fwd: vec3<f32>) -> Probe {
    let clip = params.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    let uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    let on_screen = clip.w > 0.0 && all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    let v = texel(t_velocity, uv);
    let valid = on_screen && v.z < SKY_DEPTH;
    let depth = dot(fwd, p - params.camera_pos.xyz);
    return Probe(depth - v.z, uv, valid, on_screen);
}

// March the reflected ray `r` from `origin` in `steps` steps of `step` world units,
// against depths along the view axis `fwd`. Returns the colour it hits and, in `.w`,
// how much the hit counts (0: a miss).
fn trace(origin: vec3<f32>, r: vec3<f32>, fwd: vec3<f32>, steps: i32, step: f32) -> vec4<f32> {
    let thickness = step * 1.5;
    var prev = origin;
    for (var i = 0; i < steps; i = i + 1) {
        let p = origin + r * (step * f32(i + 1));
        let here = probe(p, fwd);
        if (!here.on_screen) {
            break;
        }
        if (here.valid && here.behind > 0.0 && here.behind < thickness) {
            // Bisect between the last point in front and this one behind.
            var front = prev;
            var back = p;
            for (var j = 0; j < REFINE; j = j + 1) {
                let mid = (front + back) * 0.5;
                let m = probe(mid, fwd);
                if (m.valid && m.behind > 0.0) {
                    back = mid;
                } else {
                    front = mid;
                }
            }
            let hit = probe(back, fwd);
            let edge = min(min(hit.uv.x, 1.0 - hit.uv.x), min(hit.uv.y, 1.0 - hit.uv.y));
            // Explicit-LOD sample: a gradient instruction inside this break-bounded
            // (varying-iteration) loop fails to compile under D3D12/FXC.
            let colour = textureSampleLevel(t_color, s_color, hit.uv, 0.0).rgb;
            return vec4<f32>(colour, clamp(edge / SCREEN_FADE, 0.0, 1.0));
        }
        prev = p;
    }
    return vec4<f32>(0.0);
}

@fragment
fn fs_ssr(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let uv = pos.xy / vec2<f32>(textureDimensions(t_color));
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let v = textureLoad(t_velocity, px, 0);
    // Rough, unlit or the sky: left as it is.
    if (!ssr_surface::reflects(s) || v.z >= SKY_DEPTH) {
        discard;
    }
    let fwd = view_forward();
    let world = world_at(uv, v.z, fwd);
    let r = reflect(normalize(world - params.camera_pos.xyz), s.normal);
    let origin = world + s.normal * 0.02;
    let hit = trace(origin, r, fwd, i32(params.march.x), params.march.y);
    if (hit.w <= 0.0) {
        discard;
    }
    let c = textureLoad(t_color, px, 0);
    return vec4<f32>(max(c.rgb + (s.weight * hit.rgb - s.applied) * hit.w, vec3<f32>(0.0)), c.a);
}
