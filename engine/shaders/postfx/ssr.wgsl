// engine/shaders/postfx/ssr.wgsl — screen-space reflections (#1521, #1526): one
// fullscreen pass over the base camera's opaque image, run right after its opaque
// pass (`render::postfx::effects::ssr::pass`), before the translucents, the overlay
// cameras and every post effect. High tier only; on every tier the forward pass
// reflects the sky or a reflection probe (#718).
//
// It reads three targets of the view's own: the velocity target (#1435) for each
// pixel's depth along the view axis (`.z`; the sky at `SKY_DEPTH`), and the two SSR
// surface targets (`lighting/ssr_surface.wgsl`) for its shading normal, the weight a
// hit gets, the environment reflection the forward pass applied and its roughness. A
// surface smoother than the volume's *Minimum Smoothness* reflects, fading in over
// `ssr_surface::SMOOTHNESS_FADE` (#1736), and reads its hit from the mip pyramid of
// `t_color` at its GGX lobe's width (`ssr_march::trace`'s cone), so a rough surface
// shows a soft reflection of what stands on it and a mirror a sharp one.
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
// nothing from the march. A pixel that gets no SSR is discarded: the pass loads the
// image and leaves it as it was.
//
// The march itself is `lighting/ssr_march.wgsl`, shared with a transparent surface
// that traces its own (#1570). No derivatives here (src/render/CLAUDE.md, *Shaders*):
// every read is a `textureLoad` or an explicit-LOD sample, so the per-pixel branches
// below are safe.

#import fullscreen
#import lighting::ssr_surface
#import lighting::ssr_march

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
// A copy of the opaque image, which the pass draws over, with its mip pyramid
// (`render::postfx::effects::ssr::pyramid`), read through a trilinear sampler.
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

@fragment
fn fs_ssr(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let px = vec2<i32>(pos.xy);
    let uv = pos.xy / vec2<f32>(textureDimensions(t_color));
    let s = ssr_surface::decode(textureLoad(t_weight, px, 0), textureLoad(t_applied, px, 0));
    let v = textureLoad(t_velocity, px, 0);
    let fade = ssr_surface::smoothness_fade(s.roughness, params.surface.x, params.surface.y);
    // Rough, unlit or the sky: left as it is.
    if (!ssr_surface::reflects(s) || fade <= 0.0 || v.z >= ssr_march::SKY_DEPTH) {
        discard;
    }
    let fwd = view_forward();
    let world = world_at(uv, v.z, fwd);
    let r = reflect(normalize(world - params.camera_pos.xyz), s.normal);
    // A normal map can tilt the ray below the surface itself (a rough floor seen at a
    // grazing angle): the march would hit the floor it left and show its own colour.
    // Such a ray keeps the probe reflection, as a miss does.
    if (dot(r, geometric_normal(px, world, fwd, s.normal)) < 0.0) {
        discard;
    }
    let origin = world + s.normal * 0.02;
    let f = ssr_march::Frame(params.view_proj, params.camera_pos.xyz, fwd, vec4<f32>(0.0, 0.0, 1.0, 0.0));
    let hit = ssr_march::trace(f, t_velocity, t_color, s_color, origin, r, i32(params.march.x), params.march.y, cone(s.roughness));
    let k = hit.w * fade;
    if (k <= 0.0) {
        discard;
    }
    let c = textureLoad(t_color, px, 0);
    return vec4<f32>(max(c.rgb + (s.weight * hit.rgb - s.applied) * k, vec3<f32>(0.0)), c.a);
}
