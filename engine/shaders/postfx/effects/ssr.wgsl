// engine/shaders/postfx/effects/ssr.wgsl — screen-space reflections, on the HDR image
// before bloom (High tier only; on every tier the forward pass reflects the sky or a
// reflection probe, #718). Slot 0: x the mode (`2`: screen-space, else off), y the
// march's step count and z its step length in world units, from the volume's quality
// (`render::postfx::effects::ssr::march`).
//
// It reads three targets of the view's own (#1526): the velocity target (#1435) for
// each pixel's depth along its camera's view axis and which camera of the stack drew
// it, and the two SSR surface targets (`lighting/ssr_surface.wgsl`) for its shading
// normal, the weight a hit gets and the environment reflection the forward pass
// applied. So only a smooth surface reflects, only the base camera's pixels do (an
// overlay camera's viewmodel gets none, and hides the world under it from the march),
// and the march still finds the world after a `DepthOnly` overlay cleared the depth
// buffer.
//
// An SSR hit **replaces** the environment reflection the forward pass applied, sky or
// reflection probe alike, as HDRP's reflection hierarchy does: `pixel − applied +
// weight × hit`, both already through the pixel's `ao` and fog. A miss leaves the
// pixel as the forward pass lit it, so SSR never samples the sky itself (#1542).
//
// No derivatives here (src/render/CLAUDE.md, *Shaders*): every read is a
// `textureLoad` or an explicit-LOD sample, so the per-pixel branches below are safe.

#define_import_path postfx::effects::ssr

#import lighting::ssr_surface
#import postfx::frame::{Frag, params, t_color, s_color, world_from_depth}

// How far from a screen edge, as a fraction of the screen, a hit fades out (HDRP's
// *Screen Edge Fade Distance*).
const SCREEN_FADE: f32 = 0.1;
// Bisection steps that refine a hit between the last two march points.
const REFINE: i32 = 4;
// A velocity texel's depth at or beyond this is the sky (`render::motion::SKY_DISTANCE`).
const SKY_DEPTH: f32 = 60000.0;

// The base camera's view axis, from the frame's inverse view-projection.
fn view_forward() -> vec3<f32> {
    let near = world_from_depth(vec2<f32>(0.5), 0.0);
    return normalize(world_from_depth(vec2<f32>(0.5), 0.5) - near);
}

// The point at `uv` whose depth along the view axis is `depth`, for a perspective or
// orthographic camera alike: depth is affine along the pixel's ray.
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
// behind the base camera's surface there (> 0: behind), which counts only where
// `valid` (not the sky, not an overlay camera's pixel).
struct Probe {
    behind: f32,
    uv: vec2<f32>,
    valid: bool,
    on_screen: bool,
};

fn probe(velocity: texture_2d<f32>, p: vec3<f32>, fwd: vec3<f32>) -> Probe {
    let clip = params.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    let uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    let on_screen = clip.w > 0.0 && all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    let v = texel(velocity, uv);
    let valid = on_screen && v.w < 0.5 && v.z < SKY_DEPTH;
    let depth = dot(fwd, p - params.camera_pos.xyz);
    return Probe(depth - v.z, uv, valid, on_screen);
}

// March the reflected ray `r` from `origin` in `steps` steps of `step` world units.
// Returns the colour it hits and, in `.w`, how much the hit counts (0: a miss).
fn trace(velocity: texture_2d<f32>, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32) -> vec4<f32> {
    let fwd = view_forward();
    let thickness = step * 1.5;
    var prev = origin;
    for (var i = 0; i < steps; i = i + 1) {
        let p = origin + r * (step * f32(i + 1));
        let here = probe(velocity, p, fwd);
        if (!here.on_screen) {
            break;
        }
        if (here.valid && here.behind > 0.0 && here.behind < thickness) {
            // Bisect between the last point in front and this one behind.
            var front = prev;
            var back = p;
            for (var j = 0; j < REFINE; j = j + 1) {
                let mid = (front + back) * 0.5;
                let m = probe(velocity, mid, fwd);
                if (m.valid && m.behind > 0.0) {
                    back = mid;
                } else {
                    front = mid;
                }
            }
            let hit = probe(velocity, back, fwd);
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

fn hdr(c: vec3<f32>, f: Frag, velocity: texture_2d<f32>, weight: texture_2d<f32>, applied: texture_2d<f32>, p: vec4<f32>) -> vec3<f32> {
    if (p.x < 1.5) {
        return c;
    }
    let s = ssr_surface::decode(texel(weight, f.uv), texel(applied, f.uv));
    let v = texel(velocity, f.uv);
    // Rough, unlit or sky; or drawn by an overlay camera, whose matrices these are not.
    if (!ssr_surface::reflects(s) || v.w > 0.5 || v.z >= SKY_DEPTH) {
        return c;
    }
    let world = world_at(f.uv, v.z, view_forward());
    let r = reflect(normalize(world - params.camera_pos.xyz), s.normal);
    let origin = world + s.normal * 0.02;
    let hit = trace(velocity, origin, r, i32(p.y), p.z);
    if (hit.w <= 0.0) {
        return c;
    }
    return max(c + (s.weight * hit.rgb - s.applied) * hit.w, vec3<f32>(0.0));
}
