// engine/shaders/postfx/lens_flare.wgsl — the screen-space lens flare's pass
// (`render::postfx::effects::lens_flare`, #1443), Unity URP's *Screen Space Lens
// Flare*: `fs_flare` builds the ghosts, the halo and the streaks from bloom's blurred
// bright pass at a quarter of the view's size or less, which the composite adds
// (`postfx/effects/lens_flare.wgsl`, #1473). Without a bright pixel every tap reads
// zero, so the add leaves the image as it was.

#import fullscreen

// Mirrors the Rust `FlareParams`.
struct FlareParams {
    // rgb: the tint times the intensity, w: the ghost count
    tint: vec4<f32>,
    // x: ghosts, y: halo, z: streaks, w: the chromatic aberration
    strengths: vec4<f32>,
    // x: the view's aspect (width / height), yzw: unused
    frame: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: FlareParams;
@group(0) @binding(1) var t_color: texture_2d<f32>;
@group(0) @binding(2) var s_color: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) vid: u32) -> VsOut {
    var out: VsOut;
    out.uv = fullscreen::uv(vid);
    out.pos = fullscreen::triangle_down(vid);
    return out;
}

// The most ghosts a flare draws: Rust's `LensFlare::MAX_GHOSTS`, which a test holds
// equal to this and to the cases of `ghost_scale` and `ghost_tint`.
const MAX_GHOSTS: i32 = 8;

// One streak tap's step, a fraction of the screen's width, so a streak spans the
// same share of the screen at every resolution.
const STREAK_STEP: f32 = 0.008;

// What a strength of `1` adds of the bright pass: a real lens's ghosts reflect a
// few percent of the light, the streaks' taps spread it over ~8 weights' worth.
const GHOST_WEIGHT: f32 = 0.08;
const HALO_WEIGHT: f32 = 0.1;
const STREAK_WEIGHT: f32 = 0.03;

// Ghost `i`'s scale through the centre: `1` mirrors a source exactly, smaller
// ghosts sit nearer the centre. A function: naga forbids dynamic indexing of a
// function-local array.
fn ghost_scale(i: i32) -> f32 {
    switch i {
        case 0: { return 1.0; }
        case 1: { return 0.55; }
        case 2: { return 1.6; }
        case 3: { return 0.3; }
        case 4: { return 2.4; }
        case 5: { return 0.8; }
        case 6: { return 1.25; }
        default: { return 0.42; }
    }
}

// Ghost `i`'s colour, the lens coating's: each ghost a different hue.
fn ghost_tint(i: i32) -> vec3<f32> {
    switch i {
        case 0: { return vec3<f32>(1.0, 0.85, 0.55); }
        case 1: { return vec3<f32>(0.45, 0.85, 1.0); }
        case 2: { return vec3<f32>(0.85, 0.55, 1.0); }
        case 3: { return vec3<f32>(1.0, 0.6, 0.55); }
        case 4: { return vec3<f32>(0.6, 1.0, 0.65); }
        case 5: { return vec3<f32>(0.55, 0.65, 1.0); }
        case 6: { return vec3<f32>(1.0, 0.95, 0.75); }
        default: { return vec3<f32>(0.75, 0.85, 1.0); }
    }
}

// How much of a tap at `uv` counts: none off the image (the clamp would smear its
// edge), fading in from its border.
fn window(uv: vec2<f32>) -> f32 {
    let edge = max(abs(uv.x - 0.5), abs(uv.y - 0.5));
    return 1.0 - smoothstep(0.42, 0.5, edge);
}

// The bright pass at `uv`, its red and blue taps pushed `ca` apart along `dir`.
fn split(uv: vec2<f32>, dir: vec2<f32>, ca: f32) -> vec3<f32> {
    let off = dir * ca * 0.03;
    let r = textureSampleLevel(t_color, s_color, uv - off, 0.0).r;
    let g = textureSampleLevel(t_color, s_color, uv, 0.0).g;
    let b = textureSampleLevel(t_color, s_color, uv + off, 0.0).b;
    return vec3<f32>(r, g, b);
}

@fragment
fn fs_flare(in: VsOut) -> @location(0) vec4<f32> {
    let to_centre = vec2<f32>(0.5) - in.uv;
    let ca = params.strengths.w;
    var out = vec3<f32>(0.0);

    // Ghosts: the image mirrored through the centre at each ghost's scale.
    let count = i32(params.tint.w);
    for (var i = 0; i < MAX_GHOSTS; i = i + 1) {
        if (i >= count) {
            break;
        }
        let at = vec2<f32>(0.5) + to_centre / ghost_scale(i);
        out += split(at, to_centre, ca) * ghost_tint(i) * window(at) * GHOST_WEIGHT * params.strengths.x;
    }

    // Halo: a ring around the centre, lit by what lies a fixed (round) distance
    // towards the centre, strongest for sources near it.
    let aspect = vec2<f32>(params.frame.x, 1.0);
    let towards = to_centre * aspect;
    if (params.strengths.y > 0.0 && dot(towards, towards) > 0.0) {
        let at = in.uv + normalize(towards) * 0.45 / aspect;
        let near = 1.0 - min(length((at - vec2<f32>(0.5)) * aspect) / 0.75, 1.0);
        let lit = split(at, to_centre, ca) * pow(near, 4.0) * window(at);
        out += lit * HALO_WEIGHT * params.strengths.y;
    }

    // Streaks: a long horizontal smear of each bright pixel, the anamorphic lens's.
    if (params.strengths.z > 0.0) {
        var streak = vec3<f32>(0.0);
        for (var j = -12; j <= 12; j = j + 1) {
            let at = in.uv + vec2<f32>(f32(j) * STREAK_STEP, 0.0);
            let w = exp(-abs(f32(j)) / 4.0);
            streak += textureSampleLevel(t_color, s_color, at, 0.0).rgb * w * window(at);
        }
        out += streak * STREAK_WEIGHT * params.strengths.z;
    }

    return vec4<f32>(out * params.tint.rgb, 0.0);
}
