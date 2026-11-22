// `Frame.fwd`: the view axis for the velocity target's `.z`, or zero for the opaque copy's eye
// distance (`.a`). No derivatives: a march may run in a per-pixel branch.

#define_import_path lighting::ssr_march

// The edge fade, as a screen fraction (HDRP's *Screen Edge Fade Distance*).
const SCREEN_FADE: f32 = 0.1;
const REFINE: i32 = 4;
// Sky depths start here, under both targets' `SKY_DISTANCE` (65504).
const SKY_DEPTH: f32 = 60000.0;

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

const REACH_FADE: f32 = 0.2;

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

// A step's longest move, as a fraction of the screen's shorter side: a fixed world step near
// the camera skips thin objects and grazing floors.
const SCREEN_STEP: f32 = 1.0 / 64.0;

fn step_cap(steps: i32, dim: vec2<f32>) -> f32 {
    return max(SCREEN_STEP, 1.0 / f32(max(steps, 1))) * min(dim.x, dim.y);
}

// The depth target holds only front faces, so `THICKNESS` is how deep (m) an object
// is taken to be. `DEPTH_SLACK` is a texel's own depth on a surface seen near grazing: 2%
// holds a floor down to a few degrees at 1080p.
const THICKNESS: f32 = 0.15;
const DEPTH_SLACK: f32 = 0.02;

// A hit fades to the probe as its blur (on-screen cone radius over the shorter side) runs
// between these: a wider blur reads as a block in a hard edge.
const BLUR_FADE_START: f32 = 0.04;
const BLUR_FADE_END: f32 = 0.12;

fn project(f: Frame, p: vec3<f32>) -> vec3<f32> {
    let clip = f.view_proj * vec4<f32>(p, 1.0);
    let s = clip.xy / max(clip.w, 1e-6);
    return vec3<f32>(s.x * 0.5 + 0.5, 0.5 - s.y * 0.5, clip.w);
}

fn slack(f: Frame, p: vec3<f32>) -> f32 {
    return max(THICKNESS, depth_of(f, p) * DEPTH_SLACK);
}

const FRONT: i32 = 0;
const ON: i32 = 1;
const BEYOND: i32 = 2;

fn state(f: Frame, here: Probe, p: vec3<f32>) -> i32 {
    if (!here.valid || here.behind <= 0.0) {
        return FRONT;
    }
    return select(BEYOND, ON, here.behind <= slack(f, p));
}

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

// `cone`: the GGX lobe's half-angle tangent (0 a mirror); the hit reads the `t_color` mip
// matching its on-screen width.
fn trace(f: Frame, t_depth: texture_2d<f32>, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32, cone: f32) -> vec4<f32> {
    return trace_hit(f, t_depth, t_color, s_color, origin, r, steps, step, cone, 1.0).colour;
}

struct Hit {
    colour: vec4<f32>,
    dist: f32,
};

fn trace_hit(f: Frame, t_depth: texture_2d<f32>, t_color: texture_2d<f32>, s_color: sampler, origin: vec3<f32>, r: vec3<f32>, steps: i32, step: f32, cone: f32, blur_scale: f32) -> Hit {
    let depth_dim = vec2<f32>(textureDimensions(t_depth));
    let cap = step_cap(steps, depth_dim);
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
            // The halving may settle on another edge: a step landing on the scene still hits there.
            if (state(f, hit, back) != ON && now == ON) {
                back = p;
                hit = here;
            }
            if (state(f, hit, back) == ON) {
                let dim = vec2<f32>(textureDimensions(t_color));
                let t = distance(origin, back);
                let width = footprint(f, back, r, t, cone, hit.uv, dim);
                let top = f32(textureNumLevels(t_color) - 1u);
                let lod = clamp(log2(max(width * blur_scale, 1.0)), 0.0, top);
                let edge = min(min(hit.uv.x, 1.0 - hit.uv.x), min(hit.uv.y, 1.0 - hit.uv.y));
                let blur = 0.5 * width / min(dim.x, dim.y);
                let done = f32(i) + distance(prev, back) / len;
                let far = clamp((f32(steps) - done) / (f32(steps) * REACH_FADE), 0.0, 1.0);
                let wide = clamp((BLUR_FADE_END - blur) / (BLUR_FADE_END - BLUR_FADE_START), 0.0, 1.0);
                // The tested depth texel's centre: a refined hit sits on its edge, where a
                // filtered read bleeds the neighbour.
                let centre = (floor(clamp(hit.uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * depth_dim) + 0.5) / depth_dim;
                let colour = textureSampleLevel(t_color, s_color, centre, lod).rgb;
                return Hit(vec4<f32>(colour, clamp(edge / (SCREEN_FADE + blur), 0.0, 1.0) * far * wide), t);
            }
        }
        prev = p;
        prev_uv = here.uv;
        was = now;
    }
    return Hit(vec4<f32>(0.0), 0.0);
}
