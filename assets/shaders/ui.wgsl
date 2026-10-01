// ui.wgsl — the in-game UI pass (#418), with SDF text (#419).
//
// Draws laid-out UI graphics onto the finished frame, after post-FX. Vertices
// arrive in NDC with a straight-alpha tint in display (sRGB-encoded) space. The
// pass targets a NON-sRGB view of the frame, so blending happens on the encoded
// values — the way designers author UI: a 50% alpha looks as it does in an image
// editor. Textures are sRGB, so sampling decodes them to linear; they are
// re-encoded here before the tint multiplies them. Output is premultiplied
// (blend: One, OneMinusSrcAlpha).
//
// World canvases (#429: `WorldSpace`, `ScreenSpaceCamera`) use `vs_world` /
// `fs_world` instead: the same vertices, their NDC mapped onto the canvas plane in
// the scene (`world_ui.to_world`) and projected by the camera, drawn into the HDR
// scene target before post-FX, depth-tested against the world and fogged like it
// (the shared `fog_factor`, #437). The HDR target is linear, so the display-space
// result is decoded before it fogs and blends.
//
// Every batch, overlay or world, is clipped here per fragment (`clip_coverage`):
// its RectMask bounds arrive as canvas-NDC bounds (#619 — a scissor cannot follow
// a plane in perspective), softened over each edge's feather (#428), times the
// nearest Mask's coverage texture (#428), sampled at the fragment's canvas
// position. Batches with nothing to clip bind a white mask and unbounded bounds.
// `fs_mask` renders a Mask's graphic into that texture; `fs_backdrop` (#426) shows
// the blurred, filtered frame through a graphic's shape on overlay canvases.
//
// Text vertices (`sdf.x` = 1) sample a single-channel signed distance field
// instead: 0.5 is the glyph edge and the field reaches SPREAD atlas pixels either
// side. Fill, outline and glow are all cut from that one distance, antialiased
// over one screen pixel (fwidth), and composited glow < outline < fill.
//
// Shape vertices (`sdf.x` = 2, #425) evaluate an analytic distance field of the
// shape at the fragment's rect-local position (`local.xy`, reference units from
// the rect's centre; `local.zw` the half size): fill inside the border band,
// `border` (in `outline`) across it. The same field draws the shape's shadow
// (softened over `sdf.z`) and outer glow (fading over `sdf.w` past the edge) on
// their own quads. Any graphic's tint may be a gradient (`grad.x` > 0): up to four
// stops (`color`, `stop1..3` at `stop_t`) swept linearly or radially over the rect.

#import common::{Fog, fog_factor}

// Must match `render::ui::text::sdf::SPREAD` (a unit test checks it).
const SPREAD: f32 = 12.0;

struct VertexIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) outline: vec4<f32>,
    @location(4) glow: vec4<f32>,
    @location(5) sdf: vec4<f32>,
    @location(6) local: vec4<f32>,
    @location(7) shape: vec4<f32>,
    @location(8) radii: vec4<f32>,
    @location(9) grad: vec4<f32>,
    @location(10) stop1: vec4<f32>,
    @location(11) stop2: vec4<f32>,
    @location(12) stop3: vec4<f32>,
    @location(13) stop_t: vec4<f32>,
};

struct VertexOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) outline: vec4<f32>,
    @location(3) glow: vec4<f32>,
    @location(4) sdf: vec4<f32>,
    // World canvases only: the fragment's world position, for the fog, and its
    // canvas NDC, for the RectMask clip.
    @location(5) world: vec3<f32>,
    @location(6) canvas: vec2<f32>,
    @location(7) local: vec4<f32>,
    @location(8) shape: vec4<f32>,
    @location(9) radii: vec4<f32>,
    @location(10) grad: vec4<f32>,
    @location(11) stop1: vec4<f32>,
    @location(12) stop2: vec4<f32>,
    @location(13) stop3: vec4<f32>,
    @location(14) stop_t: vec4<f32>,
};

// One batch's clip and backdrop: the RectMask bounds (canvas NDC: min.xy, max.xy),
// each edge's feather (canvas NDC: left, bottom, right, top; 0 = hard), and a
// backdrop's tint and [saturation, brightness].
struct UiBatch {
    clip: vec4<f32>,
    feather: vec4<f32>,
    tint: vec4<f32>,
    grade: vec4<f32>,
};

// A world canvas's placement for one camera: canvas NDC → world, world → clip;
// the camera position and the scene fog.
struct WorldUi {
    view_proj: mat4x4<f32>,
    to_world: mat4x4<f32>,
    eye: vec4<f32>,
    fog: Fog,
};

@group(0) @binding(0) var ui_texture: texture_2d<f32>;
@group(0) @binding(1) var ui_sampler: sampler;
@group(1) @binding(0) var<uniform> batch: UiBatch;
@group(1) @binding(1) var mask_texture: texture_2d<f32>;
@group(1) @binding(2) var backdrop_texture: texture_2d<f32>;
@group(1) @binding(3) var effect_sampler: sampler;
@group(2) @binding(0) var<uniform> world_ui: WorldUi;

fn pass_through(in: VertexIn, clip: vec4<f32>, world: vec3<f32>) -> VertexOut {
    var out: VertexOut;
    out.clip = clip;
    out.world = world;
    out.canvas = in.pos;
    out.uv = in.uv;
    out.color = in.color;
    out.outline = in.outline;
    out.glow = in.glow;
    out.sdf = in.sdf;
    out.local = in.local;
    out.shape = in.shape;
    out.radii = in.radii;
    out.grad = in.grad;
    out.stop1 = in.stop1;
    out.stop2 = in.stop2;
    out.stop3 = in.stop3;
    out.stop_t = in.stop_t;
    return out;
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    return pass_through(in, vec4<f32>(in.pos, 0.0, 1.0), vec3<f32>(0.0));
}

@vertex
fn vs_world(in: VertexIn) -> VertexOut {
    let world = world_ui.to_world * vec4<f32>(in.pos, 0.0, 1.0);
    return pass_through(in, world_ui.view_proj * world, world.xyz);
}

// sRGB → linear transfer (IEC 61966-2-1), per channel.
fn decode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((max(c, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// Linear → sRGB transfer (IEC 61966-2-1), per channel.
fn encode_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

// A straight-alpha colour at `coverage`, premultiplied.
fn premul(c: vec4<f32>, coverage: f32) -> vec4<f32> {
    let a = c.a * coverage;
    return vec4<f32>(c.rgb * a, a);
}

// Fill over outline over glow, from the signed distance `d` (atlas pixels,
// inside positive) with an antialiasing half-width `aa`.
fn sdf_text(in: VertexOut, d: f32, aa: f32) -> vec4<f32> {
    let body = d + in.sdf.y;
    let edge = body + in.sdf.z;
    let fill = premul(in.color, smoothstep(-aa, aa, body));
    let outline = premul(in.outline, smoothstep(-aa, aa, edge));
    var c = fill + outline * (1.0 - fill.a);
    let reach = max(in.sdf.w, 1e-4);
    let falloff = 1.0 - clamp(-edge / reach, 0.0, 1.0);
    let glow = premul(in.glow, falloff * falloff);
    return c + glow * (1.0 - c.a);
}

// The tint at this fragment: the flat colour, or the gradient's.
fn tint(in: VertexOut) -> vec4<f32> {
    if in.grad.x < 0.5 {
        return in.color;
    }
    // The rect as a unit square, (0, 0) bottom-left.
    let f = in.local.xy / max(in.local.zw, vec2<f32>(1e-4)) * 0.5 + 0.5;
    var t: f32;
    if in.grad.x < 1.5 {
        t = dot(f - 0.5, in.grad.yz) / in.grad.w * 0.5 + 0.5;
    } else {
        t = length(f - in.grad.yz) / in.grad.w;
    }
    t = clamp(t, 0.0, 1.0);
    let s = in.stop_t;
    var c = mix(in.color, in.stop1, clamp((t - s.x) / max(s.y - s.x, 1e-5), 0.0, 1.0));
    c = mix(c, in.stop2, clamp((t - s.y) / max(s.z - s.y, 1e-5), 0.0, 1.0));
    return mix(c, in.stop3, clamp((t - s.z) / max(s.w - s.z, 1e-5), 0.0, 1.0));
}

// The graphic's premultiplied display-space colour.
fn shade(in: VertexOut) -> vec4<f32> {
    // Sampled and differentiated unconditionally: both stay in uniform control flow.
    let texel = textureSample(ui_texture, ui_sampler, in.uv);
    let d = (texel.r - 0.5) * 2.0 * SPREAD;
    let aa = max(fwidth(d), 1e-4) * 0.5;
    let ds = shape_distance(in);
    let aa_shape = max(fwidth(ds), 1e-4) * 0.5;
    let c = vec4<f32>(encode_srgb(texel.rgb), texel.a) * tint(in);
    let image = vec4<f32>(c.rgb * c.a, c.a);
    let text = select(image, sdf_text(in, d, aa), in.sdf.x > 0.5);
    return select(text, sdf_shape(in, ds, aa_shape), in.sdf.x > 1.5);
}

// ---- Shapes (#425) --------------------------------------------------------

// Distance from `p` to the segment `a`–`b`.
fn segment_distance(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let ab = b - a;
    let h = clamp(dot(p - a, ab) / max(dot(ab, ab), 1e-8), 0.0, 1.0);
    return length(p - a - ab * h);
}

// This quadrant's corner size from `r` (top-left, top-right, bottom-right,
// bottom-left), kept within the half size `h`.
fn corner(p: vec2<f32>, h: vec2<f32>, r: vec4<f32>) -> f32 {
    let left = p.x < 0.0;
    let c = select(select(r.z, r.w, left), select(r.y, r.x, left), p.y > 0.0);
    return clamp(c, 0.0, min(h.x, h.y));
}

fn rounded_rect(p: vec2<f32>, h: vec2<f32>, r: f32) -> f32 {
    let q = abs(p) - h + vec2<f32>(r);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

fn chamfered_rect(p: vec2<f32>, h: vec2<f32>, c: f32) -> f32 {
    let q = abs(p) - h;
    let box = min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0)));
    let cut = (q.x + q.y + c) * 0.70710678;
    return select(box, max(box, cut), c > 0.0);
}

// Inigo Quilez's ellipse approximation: exact on the edge, close enough near it.
fn ellipse(p: vec2<f32>, h: vec2<f32>) -> f32 {
    let r = max(h, vec2<f32>(1e-4));
    let k0 = length(p / r);
    let k1 = length(p / (r * r));
    return select(k0 * (k0 - 1.0) / k1, -min(r.x, r.y), k1 < 1e-6);
}

// A band from radius `inner` to the half size's smaller side, swept clockwise
// from 12 o'clock between `start` and `end` degrees with flat ends.
fn ring(p: vec2<f32>, h: vec2<f32>, params: vec3<f32>) -> f32 {
    let outer = min(h.x, h.y);
    let inner = clamp(params.x, 0.0, outer);
    let len = length(p);
    let band = select(len - outer, max(len - outer, inner - len), inner > 0.0);
    let span = params.z - params.y;
    if span >= 360.0 {
        return band;
    }
    if span <= 0.0 {
        return 1e5;
    }
    let deg = 57.29578;
    let angle = atan2(p.x, p.y) * deg;
    let rel = ((angle - params.y) % 360.0 + 360.0) % 360.0;
    let a0 = params.y / deg;
    let a1 = params.z / deg;
    let d0 = vec2<f32>(sin(a0), cos(a0));
    let d1 = vec2<f32>(sin(a1), cos(a1));
    let caps = min(
        segment_distance(p, d0 * inner, d0 * outer),
        segment_distance(p, d1 * inner, d1 * outer),
    );
    return select(caps, max(band, -caps), rel <= span);
}

// A `thickness` line along x across the rect, dashed when `dash` > 0.
fn line(p: vec2<f32>, h: vec2<f32>, params: vec3<f32>) -> f32 {
    let dy = abs(p.y) - params.x * 0.5;
    var dx = abs(p.x) - h.x;
    let period = params.y + params.z;
    if params.y > 0.0 && period > 0.0 {
        let u = ((p.x + h.x) % period + period) % period;
        let in_dash = max(-u, u - params.y);
        let in_gap = min(u - params.y, period - u);
        dx = max(dx, select(in_gap, in_dash, u <= params.y));
    }
    return min(max(dx, dy), 0.0) + length(max(vec2<f32>(dx, dy), vec2<f32>(0.0)));
}

// The signed distance (negative inside, reference units) to this vertex's shape.
fn shape_distance(in: VertexOut) -> f32 {
    let p = in.local.xy;
    let h = in.local.zw;
    let kind = in.shape.x;
    if kind < 0.5 {
        return rounded_rect(p, h, corner(p, h, in.radii));
    } else if kind < 1.5 {
        return chamfered_rect(p, h, corner(p, h, in.radii));
    } else if kind < 2.5 {
        return ellipse(p, h);
    } else if kind < 3.5 {
        return ring(p, h, in.shape.yzw);
    }
    return line(p, h, in.shape.yzw);
}

// Coverage of the inside of an edge at distance `d`, antialiased over `aa`.
fn inside(d: f32, aa: f32) -> f32 {
    return 1.0 - smoothstep(-aa, aa, d);
}

// A shape quad's premultiplied colour: the glow halo (`sdf.w` > 0), else the fill
// inside the border band (softened over `sdf.z` for a shadow) and the border on it.
fn sdf_shape(in: VertexOut, d: f32, aa: f32) -> vec4<f32> {
    let reach = in.sdf.w;
    if reach > 0.0 {
        let fade = 1.0 - clamp(d / reach, 0.0, 1.0);
        return premul(tint(in), fade * fade * (1.0 - inside(d, aa)));
    }
    let soft = max(in.sdf.z, aa);
    let width = in.sdf.y;
    let body = inside(d + width, soft);
    let edge = inside(d, soft);
    return premul(tint(in), body) + premul(in.outline, max(edge - body, 0.0));
}

// A canvas-NDC point → the uv of a texture covering the canvas (mask, backdrop).
fn canvas_uv(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
}

// How much of the batch shows at canvas-NDC `p`: inside the RectMask bounds (hard,
// or faded over each edge's feather) times the Mask coverage there.
fn clip_coverage(p: vec2<f32>) -> f32 {
    let r = batch.clip;
    let inside = vec4<f32>(p.x - r.x, p.y - r.y, r.z - p.x, r.w - p.y);
    let f = batch.feather;
    let hard = select(vec4<f32>(0.0), vec4<f32>(1.0), inside >= vec4<f32>(0.0));
    let soft = clamp(inside / max(f, vec4<f32>(1e-6)), vec4<f32>(0.0), vec4<f32>(1.0));
    let k = select(hard, soft, f > vec4<f32>(0.0));
    let mask = textureSample(mask_texture, effect_sampler, canvas_uv(p)).r;
    return k.x * k.y * k.z * k.w * mask;
}

// The colour a graphic draws with, on screen and world canvases alike. A baked
// ui shader (#427, `Shader.Bake` pass "ui") replaces exactly this function with
// its blocks folded over `shade(in)`; keep its text in step with
// `shadergen::assemble::ui::GRAPHIC`. Clips and masks apply after it.
fn graphic(in: VertexOut) -> vec4<f32> {
    return shade(in);
}

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4<f32> {
    return graphic(in) * clip_coverage(in.canvas);
}

// A Mask's graphic into its coverage texture (R8): the graphic's alpha, clipped by
// the masks above it (#428).
@fragment
fn fs_mask(in: VertexOut) -> @location(0) vec4<f32> {
    return vec4<f32>(shade(in).a * clip_coverage(in.canvas), 0.0, 0.0, 0.0);
}

// A backdrop (#426): the blurred frame (display-encoded, read through a non-sRGB
// view) through the graphic's shape — saturated, brightened, then tinted.
@fragment
fn fs_backdrop(in: VertexOut) -> @location(0) vec4<f32> {
    let cover = shade(in).a * clip_coverage(in.canvas);
    var c = textureSample(backdrop_texture, effect_sampler, canvas_uv(in.canvas)).rgb;
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(luma), c, batch.grade.x) * batch.grade.y;
    c = clamp(mix(c, batch.tint.rgb, batch.tint.a), vec3<f32>(0.0), vec3<f32>(1.0));
    return vec4<f32>(c * cover, cover);
}

// Into the linear HDR target: un-premultiply, decode, fog, premultiply again.
@fragment
fn fs_world(in: VertexOut) -> @location(0) vec4<f32> {
    let c = graphic(in) * clip_coverage(in.canvas);
    let rgb = select(c.rgb / max(c.a, 1e-6), vec3<f32>(0.0), c.a <= 0.0);
    let f = fog_factor(world_ui.fog, in.world, world_ui.eye.xyz);
    let lit = mix(decode_srgb(rgb), world_ui.fog.color, f);
    return vec4<f32>(lit * c.a, c.a);
}
