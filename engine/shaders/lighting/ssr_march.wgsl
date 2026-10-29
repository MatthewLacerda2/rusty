// engine/shaders/lighting/ssr_march.wgsl — the screen-space reflection march (#1521),
// shared by the SSR pass over the opaque image (`postfx/ssr.wgsl`) and a transparent
// surface that traces its own (`forward/transparent.wgsl`'s `reflect_ssr`, #1570).
// One march, two depth sources: the SSR pass reads each pixel's depth along the view
// axis from the velocity target's `.z`; a transparent reads the eye distance in the
// opaque copy's `.a` (`opaque_copy.wgsl`). A `Frame` says which.
//
// No derivatives here (src/render/CLAUDE.md, *Shaders*): every read is a
// `textureLoad` or an explicit-LOD sample, so a caller may march in a per-pixel branch.

#define_import_path lighting::ssr_march

// How far from a screen edge, as a fraction of the screen, a hit fades out (HDRP's
// *Screen Edge Fade Distance*).
const SCREEN_FADE: f32 = 0.1;
// Bisection steps that refine a hit between the last two march points.
const REFINE: i32 = 4;
// A depth at or beyond this is the sky: the velocity target's
// `render::motion::SKY_DISTANCE`, the opaque copy's `SKY_DISTANCE` (65504).
const SKY_DEPTH: f32 = 60000.0;

// What a march reads the scene against: the camera's view-projection and eye, and
// how a point's depth is measured. `fwd` is the view axis, a depth along it (the
// velocity target), or zero for the distance from the eye (the opaque copy);
// `channel` picks the depth texel's lane that holds it.
struct Frame {
    view_proj: mat4x4<f32>,
    eye: vec3<f32>,
    fwd: vec3<f32>,
    channel: vec4<f32>,
};

fn texel(t: texture_2d<f32>, uv: vec2<f32>) -> vec4<f32> {
    let dim = vec2<f32>(textureDimensions(t));
    let coord = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * dim);
    return textureLoad(t, coord, 0);
}

fn depth_of(f: Frame, p: vec3<f32>) -> f32 {
    if (all(f.fwd == vec3<f32>(0.0))) {
        return distance(p, f.eye);
    }
    return dot(f.fwd, p - f.eye);
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

fn probe(f: Frame, t_depth: texture_2d<f32>, p: vec3<f32>) -> Probe {
    let clip = f.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    let uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    let on_screen = clip.w > 0.0 && all(uv >= vec2<f32>(0.0)) && all(uv <= vec2<f32>(1.0));
    let scene = dot(texel(t_depth, uv), f.channel);
    let valid = on_screen && scene < SKY_DEPTH;
    return Probe(depth_of(f, p) - scene, uv, valid, on_screen);
}

// March the reflected ray `r` from `origin` in `steps` steps of `step` world units,
// against `t_depth`'s depths as `f` measures them. Returns the colour of `t_color`
// it hits and, in `.w`, how much the hit counts (0: a miss).
fn trace(f: Frame, t_depth: texture_2d<f32>, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32) -> vec4<f32> {
    let thickness = step * 1.5;
    var prev = origin;
    for (var i = 0; i < steps; i = i + 1) {
        let p = origin + r * (step * f32(i + 1));
        let here = probe(f, t_depth, p);
        if (!here.on_screen) {
            break;
        }
        if (here.valid && here.behind > 0.0 && here.behind < thickness) {
            // Bisect between the last point in front and this one behind.
            var front = prev;
            var back = p;
            for (var j = 0; j < REFINE; j = j + 1) {
                let mid = (front + back) * 0.5;
                let m = probe(f, t_depth, mid);
                if (m.valid && m.behind > 0.0) {
                    back = mid;
                } else {
                    front = mid;
                }
            }
            let hit = probe(f, t_depth, back);
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
