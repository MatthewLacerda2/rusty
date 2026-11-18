// engine/shaders/postfx/ssr.wgsl — screen-space reflections (#1521, #1526): two
// fullscreen passes over the base camera's opaque image, run right after its opaque
// pass (`render::postfx::effects::ssr::pass`), before the translucents, the overlay
// cameras and every post effect. High tier only; on every tier the forward pass
// reflects the sky or a reflection probe (#718).
//
// It reads three targets of the view's own: the velocity target (#1435) for each
// pixel's depth along the view axis (`.z`; the sky at `SKY_DEPTH`), and the two SSR
// surface targets (`lighting/ssr_surface.wgsl`) for its shading normal, the weight a
// hit gets, the environment reflection the forward pass applied and its roughness. A
// surface smoother than the volume's *Minimum Smoothness* reflects, fading in over
// `ssr_surface::SMOOTHNESS_FADE` (#1736). The trace (`fs_trace`) marches its ray and
// writes the hit, sharp, into a target of its own; the resolve (`fs_resolve`, #2028)
// averages the hits over the disk its GGX lobe (`cone`) spreads a reflection across,
// so a rough surface shows a soft reflection of what stands on it, its silhouette
// faded across the blur, and a mirror a sharp one.
//
// An SSR hit **replaces** the environment reflection the forward pass applied, sky or
// reflection probe alike, as HDRP's reflection hierarchy does: `pixel − applied +
// weight × hit`, both already through the pixel's `ao` and fog, blended in by the
// hit's confidence times the smoothness fade. A miss leaves the pixel as the forward
// pass lit it, so SSR never samples the sky itself (#1542), and a weak hit falls back
// to that same probe reflection continuously, with no seam.
//
// Running here, not in the post chain, is HDRP's order: the translucents, particles
// and world canvases blend over the reflected image, so their coverage scales the
// reflection like any other light under them; depth of field and motion blur blur
// it; an overlay camera (a viewmodel) has not drawn yet, so it gets none and hides
// nothing from the march. A pixel that gets no SSR is discarded: the resolve loads the
// image and leaves it as it was.
//
// The march itself is `lighting/ssr_march.wgsl`, shared with a transparent surface
// that traces its own (#1570). No derivatives here (crates/render/src/CLAUDE.md, *Shaders*):
// every read is a `textureLoad` or an explicit-LOD sample, so the per-pixel branches
// below are safe.

#import fullscreen
#import lighting::ssr_surface
#import lighting::ssr_march
#import numeric

struct SsrParams {
    inv_view_proj: mat4x4<f32>,
    view_proj: mat4x4<f32>,
    // xyz: the base camera's eye
    camera_pos: vec4<f32>,
    // x: the march's step count, y: its step length in world units, from the
    // volume's quality (`render::postfx::effects::ssr::march`)
    march: vec4<f32>,
    // x: the volume's *Minimum Smoothness* (#1736), y: the smoothness over which SSR
    // fades in above it (`ssr::fade`)
    surface: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: SsrParams;
// A copy of the opaque image, which the pass draws over, one level
// (`render::postfx::effects::ssr::pyramid::colour_copy`). The sampler is trilinear:
// the resolve reads the hit pyramids through it too.
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

// Up to this roughness a surface reads its hit sharp, from the pyramid's first level,
// at any resolution; its cone widens to the full lobe's over the next `NEAR_MIRROR`.
// The lobe's own cone would blur a near-mirror by its width on screen, which grows
// with the resolution: roughness 0.05 read about level 2 at 1080p.
const NEAR_MIRROR: f32 = 0.05;

// The tangent of the half-angle of the cone a surface of perceptual `roughness`
// reflects into, Uludag's (GPU Pro 5, *Hi-Z Screen-Space Cone-Traced Reflections*):
// GGX's `alpha = roughness²` as a Blinn-Phong power `2 / alpha² − 2`, and the cone
// that holds its lobe down to 0.244 of its peak, held to 0 up to `NEAR_MIRROR`.
// Roughness 0.05's is so 0 (the lobe's own is about 0.003); 0.4's about 0.19 (11°),
// 0.6's about 0.46 (25°).
fn cone(roughness: f32) -> f32 {
    let alpha = max(roughness * roughness, 1e-3);
    let power = 2.0 / (alpha * alpha) - 2.0;
    let c = pow(0.244, 1.0 / (power + 1.0));
    let sharp = clamp((roughness - NEAR_MIRROR) / NEAR_MIRROR, 0.0, 1.0);
    return sqrt(max(1.0 - c * c, 0.0)) / c * sharp;
}

// The world point at pixel `p`, from the velocity target's depth.
fn world_px(p: vec2<i32>, fwd: vec3<f32>) -> vec3<f32> {
    let uv = (vec2<f32>(p) + 0.5) / vec2<f32>(textureDimensions(t_velocity));
    return world_at(uv, textureLoad(t_velocity, p, 0).z, fwd);
}

// The world point at the neighbour `px + o`, clamped to the screen (at its edge, the
// pixel itself: a zero difference, which `geometric_normal` skips).
fn near(px: vec2<i32>, o: vec2<i32>, fwd: vec3<f32>) -> vec3<f32> {
    let top = vec2<i32>(textureDimensions(t_velocity)) - 1;
    return world_px(clamp(px + o, vec2<i32>(0), top), fwd);
}

// The surface's geometric normal at `px`, facing the camera, from the depths around
// it (the targets hold only the shading normal `n`); zero where it is degenerate.
// Each axis has two one-sided differences, and at an edge one of them crosses onto
// another surface (a floor pixel against a wall's foot, where the depths agree), so
// of the four normals they give, the one nearest `n` is the pixel's own surface.
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

// The radius, in pixels of `dim`, over which a hit `t` world units out is blurred on
// the surface at `world` (#2028): the cone's disk there, `c × t` across, seen in
// the mirror from `d0 + t` away (the reflected point's virtual image), so the
// radius on the surface is `c × t × d0 / (d0 + t)` at the eye distance `d0`.
fn spread(world: vec3<f32>, t: f32, c: f32, dim: vec2<f32>) -> f32 {
    return min(across(world, t, c, dim), ssr_march::BLUR_FADE_END * min(dim.x, dim.y));
}

// [`spread`] before its cap at `BLUR_FADE_END`.
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

// The cone a hit's radius is measured with for the resolve's field (#2158): the
// trace stores [`spread`] per unit of cone, measured at a typical satin cone (about
// roughness 0.4's), where the projection is close to linear, so each pixel scales
// it by its own cone. Stored no wider than `UNIT_MAX` pixels, so a hit's
// radius times its count stays inside f16.
const UNIT_CONE: f32 = 0.2;
const UNIT_MAX: f32 = 16384.0;

// What a hit's count is stored times, so the count of one hit in a coarse level of
// the spread pyramid (a 4^-8 share of it at level 8) stays a normal f16, not a
// subnormal a GPU may flush to 0 as it writes the level (#2158): the mean radius
// of a few hits in a wide window then holds, not just a crowd's.
const COUNT_SCALE: f32 = 64.0;

// What a found hit whose fades took its weight to nothing still counts for in the
// radius field, against 1 for a hit at full weight (#2158). Little enough that the
// long rays a rough surface fades out beside a contact reflection don't set that
// reflection's blur; enough that the field stays well-conditioned where the weight
// thins out: a pyramid texel of faded hits beside one of full-weight hits weighs
// too little against it at 0.1, and the mean radius between them swings over a
// few pixels, drawing a faint line across a satin reflection's fading edge.
const FOUND_FLOOR: f32 = 0.4;

// What the trace writes per pixel for the resolve (#2028): `hit`, the hit colour
// times its weight and the weight (premultiplied, so a level of its pyramid
// averages hits and misses alike); `spread`, 1 for a pixel that traced (a miss
// included), then for a ray that found something its radius per unit cone times
// its count `k`, and `k` (times `COUNT_SCALE`): its weight, but at least
// `FOUND_FLOOR` (0 and 0 for a miss), so a level averages the radius over the hits
// found, those that reflect something first (#2158).
struct Traced {
    @location(0) hit: vec4<f32>,
    @location(1) spread: vec4<f32>,
};

// Whether pixel `px` takes SSR at all: its surface reflects, is smooth enough
// (`fade` above 0) and is not the sky. Both passes ask the same.
fn takes(s: ssr_surface::SsrSurface, fade: f32, depth: f32) -> bool {
    return ssr_surface::reflects(s) && fade > 0.0 && depth < ssr_march::SKY_DEPTH;
}

// The trace (#2028's first half): marches each reflecting pixel's ray and writes
// what it hit, unblended, for the resolve. A pixel that takes no SSR writes nothing.
@fragment
fn fs_trace(@builtin(position) pos: vec4<f32>) -> Traced {
    let px = vec2<i32>(pos.xy);
    let dim = vec2<f32>(textureDimensions(t_color));
    let uv = pos.xy / dim;
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let v = textureLoad(t_velocity, px, 0);
    let fade = ssr_surface::smoothness_fade(s.roughness, params.surface.x, params.surface.y);
    // Rough, unlit or the sky: left as it is.
    if (!takes(s, fade, v.z)) {
        discard;
    }
    let fwd = view_forward();
    let world = world_at(uv, v.z, fwd);
    let r = reflect(normalize(world - params.camera_pos.xyz), s.normal);
    let missed = Traced(vec4<f32>(0.0), vec4<f32>(1.0, 0.0, 0.0, 0.0));
    // A normal map can tilt the ray below the surface itself (a rough floor seen at a
    // grazing angle): the march would hit the floor it left and show its own colour.
    // Such a ray is a miss: the resolve may fill it from the hits around it, within
    // its own cone's spread, and where none reach it keeps the probe reflection.
    if (dot(r, geometric_normal(px, world, fwd, s.normal)) < 0.0) {
        return missed;
    }
    let origin = world + s.normal * 0.02;
    let f = ssr_march::Frame(params.view_proj, params.camera_pos.xyz, fwd, vec4<f32>(0.0, 0.0, 1.0, 0.0));
    let c = cone(s.roughness);
    // Read sharp: the resolve blurs the hits by the cone, colour and edge alike.
    let hit = ssr_march::trace_hit(f, t_velocity, t_color, s_color, origin, r, i32(params.march.x), params.march.y, c, 0.0);
    if (hit.dist <= 0.0) {
        return missed;
    }
    // The colour is read from the opaque image's copy, unfiltered: an emissive past
    // f16's range is `inf` there. Such a hit reflects nothing, as a non-finite texel
    // is nothing to the hit pyramids' filter (`ssr_pyramid.wgsl`) either.
    let w = select(0.0, hit.colour.w, numeric::finite4(hit.colour));
    // A ray that found something tells the resolve how far its hit is, as a blur
    // radius per unit cone, even where its weight faded to nothing (a long ray off a
    // rough surface, a hit at the screen's edge), counted at `FOUND_FLOOR`: weighted
    // by the fades alone, the mean radius thinned out exactly where the radii are
    // widest and swung over a few pixels there.
    let unit = min(across(world, hit.dist, UNIT_CONE, dim) / UNIT_CONE, UNIT_MAX);
    let k = max(w, FOUND_FLOOR);
    let found = vec4<f32>(1.0, unit * k, k * COUNT_SCALE, 0.0);
    if (w <= 0.0) {
        return Traced(vec4<f32>(0.0), found);
    }
    return Traced(vec4<f32>(hit.colour.rgb * w, w), found);
}

// The resolve (#2028's second half, HDRP's PBR-accumulation neighbour reuse made
// deterministic): each pixel averages the trace's hits over the disk its own
// cone spreads a reflection across. A hit's colour was already read at the cone's
// width; what the average adds is where rays stop hitting: a rough surface's
// reflected silhouette fades across its blur, not at one pixel's hit or miss,
// which drew it as a stepped outline. Every pixel, hit or miss, takes its radius
// from how far the hits around it are, at its own cone (`blur_radius`), so an edge
// softens out as well as in, capped by the widest its own cone spreads (0 for a
// mirror, whose misses stay misses). Only taps whose centre lies on the same
// plane count, and the average is over pixels that traced, so a
// surface's border or a neighbouring surface mostly neither fades nor bleeds into
// it: an approximation, since a tap reads a mip texel `radius / 2` wide that can
// average in the surface beside it, so a band about that wide at a corner does.
@group(1) @binding(0) var t_hits: texture_2d<f32>;
@group(1) @binding(1) var t_spread: texture_2d<f32>;

// The disk's taps (a Vogel spiral), each read at the level of the trace's
// pyramids whose texels are as wide as the taps are apart, so the disk is covered
// with no gap to show a tap's own copy of an edge.
const TAPS: i32 = 12;
const GOLDEN_ANGLE: f32 = 2.3999632;
// Below this radius in pixels a reflection is read where it was traced (a mirror).
const SHARP: f32 = 0.5;

// Whether the surface at `q` lies on the plane through `world` facing `g`, within
// the march's own slack: a pixel of the same surface, not one beside or behind it.
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

// The mean radius per unit cone of the hits the trace found around `uv`, from
// level `lod` of its spread pyramid. A window where hits count for `trust` of its
// area or less leans to `fallback` instead, continuously (`trust` 0: none). One
// where they count for less than `LONE` of one hit's pixel fades to 0 (nothing to
// blur): a lone hit, a pixel or two in an empty window, still counts for at least
// a tenth of one there, so it keeps the radius of its own lobe.
fn mean_unit(uv: vec2<f32>, lod: f32, fallback: f32, trust: f32) -> f32 {
    let s = textureSampleLevel(t_spread, s_color, uv, lod);
    let count = s.z / COUNT_SCALE + trust;
    return (s.y + trust * fallback) / max(count, LONE * exp2(-2.0 * lod));
}

// The share of one full-count hit's pixel, at a level of the spread pyramid, under
// which `mean_unit`'s window holds nothing.
const LONE: f32 = 0.02;

// How much of a radius-wide window must hold hits before its mean radius is
// trusted over the previous, wider window's (`blur_radius`).
const RADIUS_TRUST: f32 = 0.02;
// How many times `blur_radius` narrows its window to the radius it last found.
const REFINE: i32 = 2;

// The radius, in pixels, the pixel at `uv` resolves over (#2158): this pixel's
// own cone times one field, smooth across the frame, hit and miss alike. The disk
// a pixel averages must grow and shrink no faster than the pixel moves: where the
// radius jumps (a hit taking its own ray's, its missing neighbour the hits' mean
// around it) or swings over a few pixels, the disks stop nesting and the blur
// draws that line sharp, a reflected silhouette's outline crisp around a soft
// inside. So the field is the mean radius per unit cone of the hits found around
// the pixel, read as wide as its blur can reach, then `REFINE` times over a window
// as wide as the radius last found, so a contact reflection keeps its narrower
// blur among long rays; each is a filtered read of the spread pyramid, so the
// field follows `uv` smoothly, between two hits at different distances (a bottle
// before the shelf behind it) as across a hit and a miss. A plain mean, on
// purpose: one pulled toward the nearest hits (a harmonic mean, or the narrower of
// the field and the pixel's own ray) kept a thin post's contact reflection, but
// collapsed the radius wherever a few rays were short (a seam's trim, a bottle),
// drawing those parts crisp in a soft reflection, hard blotches that moved with the
// camera. A contact reflection a few pixels wide among long rays takes their blur
// (`docs/api/Graphics.md`). Scaled by the pixel's own cone, a roughness map's
// detail and a smooth floor beside a rough wall keep their own blur. No wider
// than this pixel's cone spreads a hit as far out as the march reaches: a
// mirror's (0) is sharp, and pays for no read.
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
