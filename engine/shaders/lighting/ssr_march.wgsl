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

// How much of the march's reach, at its far end, a hit fades out over: a ray that
// hits just before running out and its neighbour that misses meet without a seam.
const REACH_FADE: f32 = 0.2;

// The on-screen diameter, in texels of `dim`, of the cone of tangent `cone` around
// the ray `r` where it hits `p`, `t` world units from where it left: the disk of
// radius `cone × t` across the ray, measured along the one direction across both the
// ray and the line of sight, which faces the camera.
fn footprint(f: Frame, p: vec3<f32>, r: vec3<f32>, t: f32, cone: f32, uv: vec2<f32>, dim: vec2<f32>) -> f32 {
    var across = cross(r, p - f.eye);
    if (dot(across, across) < 1e-10) {
        across = cross(r, select(vec3<f32>(0.0, 1.0, 0.0), vec3<f32>(1.0, 0.0, 0.0), abs(r.y) > 0.9));
    }
    let rim = f.view_proj * vec4<f32>(p + normalize(across) * (cone * t), 1.0);
    let s = rim.xy / max(rim.w, 1e-6);
    let rim_uv = vec2<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5);
    return 2.0 * length((rim_uv - uv) * dim);
}

// The longest a march step may move on screen, as a fraction of the screen's
// shorter side (#2004): a step that would move further is shortened to it. A fixed
// world step near the camera jumps tens of texels between depth taps and steps past
// a thin object (a bottle, a pole) or a grazing floor, drawing its reflection as
// stair-steps and dashes; a step held to this sees anything wider than it. The
// reach so spent is the march's, not the world step's: `trace` fades by the steps.
const SCREEN_STEP: f32 = 1.0 / 64.0;

// How far behind the scene's depth a refined hit may lie and still count (#2004):
// a ray that crosses a surface refines to within a texel of it, one that passes
// behind an object refines onto the object's edge, still behind it by however far
// it passed. The depth target holds only front faces, so a ray that meets an
// object's unseen back is behind its front by the object's own depth: `THICKNESS`
// is how deep an object is taken to be (a bottle, a pole, a crate's edge), and a ray
// further behind passes it. `DEPTH_SLACK`, a fraction of the depth, is a texel's
// own depth on a surface seen near grazing: 2% holds a floor down to a few degrees
// at 1080p (a texel's depth there is about `depth × 0.001 / tan(angle)`).
const THICKNESS: f32 = 0.15;
const DEPTH_SLACK: f32 = 0.02;

// Where a hit's blur, the cone's on-screen radius as a fraction of the screen's
// shorter side, stops being able to hide what the march cannot blur (#2004): the
// pyramid softens the hit's colour, but where a ray stops hitting (a reflected
// silhouette) is decided per pixel, so a wide blur drawn inside a hard edge reads
// as a block. From `BLUR_FADE_START` the hit fades out to the probe it replaces,
// gone at `BLUR_FADE_END`. A contact reflection (a short ray) keeps a narrow blur
// and stays; a long ray from a rough surface takes the probe.
const BLUR_FADE_START: f32 = 0.04;
const BLUR_FADE_END: f32 = 0.12;

// Where `p` lands on screen (`uv`) and whether in front of the camera: `probe`
// without the depth tap.
fn project(f: Frame, p: vec3<f32>) -> vec3<f32> {
    let clip = f.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    return vec3<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5, clip.w);
}

// How far behind the scene at `p` a point may lie and still be on it.
fn slack(f: Frame, p: vec3<f32>) -> f32 {
    return max(THICKNESS, depth_of(f, p) * DEPTH_SLACK);
}

// Where a march point stands against the scene: in front of it (or over the sky),
// on it (behind by at most `slack`: a hit), or beyond it.
const FRONT: i32 = 0;
const ON: i32 = 1;
const BEYOND: i32 = 2;

fn state(f: Frame, here: Probe, p: vec3<f32>) -> i32 {
    if (!here.valid || here.behind <= 0.0) {
        return FRONT;
    }
    return select(BEYOND, ON, here.behind <= slack(f, p));
}

// Where between `a` (of state `start`) and `b` the ray first changes state: the
// point on `b`'s side of the boundary after `REFINE` halvings. From in front, the
// boundary is where it goes behind the scene; from beyond, where it comes back
// within `slack` (or in front).
fn refine(f: Frame, t_depth: texture_2d<f32>, a: vec3<f32>, b: vec3<f32>, start: i32) -> vec3<f32> {
    var keep = a;
    var go = b;
    for (var j = 0; j < REFINE; j = j + 1) {
        let mid = (keep + go) * 0.5;
        let s = state(f, probe(f, t_depth, mid), mid);
        if (s == start) {
            keep = mid;
        } else {
            go = mid;
        }
    }
    return go;
}

// March the reflected ray `r` from `origin` in `steps` steps of at most `step`
// world units (and at most `SCREEN_STEP` of the screen), against `t_depth`'s
// depths as `f` measures them. Each point is in front of the scene, on it (behind
// by at most `slack`: an object `THICKNESS` deep) or beyond it. A step that
// leaves the front, or comes back from beyond, is refined by bisection to where it
// changed (`refine`), and hits only if that point is on the scene: one that went
// behind a thin object's edge, or came out beside it, marches on. Returns the
// colour of `t_color` it hits and, in `.w`, how much the hit counts (0: a miss):
// faded at the screen's edge, wider by the blur's radius, over the march's last
// `REACH_FADE` of its steps, and out where the blur is too wide to hide a hard
// edge (`BLUR_FADE_START`).
//
// `cone` is the tangent of the half-angle of the surface's GGX lobe (0: a mirror,
// #1736): the hit is read at the mip of `t_color` whose texels are the cone's
// on-screen width there, so a rough surface reflects a blur of what it sees (cone
// tracing, as Frostbite and UE do). A `t_color` with one level reads sharp.
fn trace(f: Frame, t_depth: texture_2d<f32>, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32, cone: f32) -> vec4<f32> {
    let depth_dim = vec2<f32>(textureDimensions(t_depth));
    let cap = SCREEN_STEP * min(depth_dim.x, depth_dim.y);
    var prev = origin;
    var prev_uv = project(f, origin).xy;
    var was = FRONT;
    for (var i = 0; i < steps; i = i + 1) {
        var len = step;
        let ahead = project(f, prev + r * step);
        let moved = length((ahead.xy - prev_uv) * depth_dim);
        if (ahead.z > 0.0 && moved > cap) {
            len = step * cap / moved;
        }
        let p = prev + r * len;
        let here = probe(f, t_depth, p);
        if (!here.on_screen) {
            break;
        }
        let now = state(f, here, p);
        if (now != was) {
            var back = refine(f, t_depth, prev, p, was);
            var hit = probe(f, t_depth, back);
            // The halving can settle on another boundary than the one the step
            // crossed (two edges between its ends): a step that landed on the scene
            // still hits there.
            if (state(f, hit, back) != ON && now == ON) {
                back = p;
                hit = here;
            }
            if (state(f, hit, back) == ON) {
                let dim = vec2<f32>(textureDimensions(t_color));
                let t = distance(origin, back);
                let width = footprint(f, back, r, t, cone, hit.uv, dim);
                let top = f32(textureNumLevels(t_color) - 1u);
                let lod = clamp(log2(max(width, 1.0)), 0.0, top);
                let edge = min(min(hit.uv.x, 1.0 - hit.uv.x), min(hit.uv.y, 1.0 - hit.uv.y));
                let blur = 0.5 * width / min(dim.x, dim.y);
                let done = f32(i) + distance(prev, back) / len;
                let far = clamp((f32(steps) - done) / (f32(steps) * REACH_FADE), 0.0, 1.0);
                let wide = clamp((BLUR_FADE_END - blur) / (BLUR_FADE_END - BLUR_FADE_START), 0.0, 1.0);
                // Explicit-LOD sample: a gradient instruction inside this break-bounded
                // (varying-iteration) loop fails to compile under D3D12/FXC.
                // At the centre of the depth texel the hit was tested against: a
                // refined hit lies on that texel's edge, and a filtered read there
                // would take half its colour from the surface beside it.
                let centre = (floor(clamp(hit.uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * depth_dim) + 0.5) / depth_dim;
                let colour = textureSampleLevel(t_color, s_color, centre, lod).rgb;
                return vec4<f32>(colour, clamp(edge / (SCREEN_FADE + blur), 0.0, 1.0) * far * wide);
            }
        }
        prev = p;
        prev_uv = here.uv;
        was = now;
    }
    return vec4<f32>(0.0);
}
