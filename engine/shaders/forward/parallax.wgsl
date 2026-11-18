// engine/shaders/forward/parallax.wgsl — a material's height map (#2191), HDRP Lit's
// *Displacement Mode: Pixel*: parallax occlusion mapping. The view ray is marched
// through the height field below the surface, and where it meets it every map of the
// surface is read instead (`displace` shifts the one `MapCoords` they all read), so
// mortar, grout and plank gaps hold their relief at a grazing angle. The depth the
// surface writes is untouched: its outline, shadows, decals, SSR, SSAO and motion
// vectors see the flat surface, and the lightmap is read at UV1, never shifted.
//
// The march is HDRP's (`ParallaxOcclusionMapping` in `PerPixelDisplacement.hlsl`,
// called from `ApplyPerPixelDisplacement` in `LitDataDisplacement.hlsl`): a linear
// search from the top (height 1, white) down, one step at a time, then one line
// intersection between the last two steps (HDRP's non-secant refinement). The step
// count follows the view's angle in the height field's own space, from HDRP's
// `_PPDMinSamples` (5) head-on to `_PPDMaxSamples` (15) at grazing; the shift fades
// out over one mip level past `_PPDLodThreshold` (5) of the height map, and a surface
// past it is not marched at all, so far surfaces pay nothing and never alias.
//
// Where HDRP scales the view by the primitive's size and an amplitude in
// centimetres, rusty takes the depth as a fraction of one repeat's width
// (`height_scale`) and finds the world-to-UV mapping from screen derivatives
// (Schüler's cotangent frame), so UV0 and Planar march alike, under any tiling,
// with no mesh tangent and no world size to know.

#define_import_path forward::parallax

#import forward::bindings::{camera, material, s_height, t_height}
#import forward::mapping::{MapCoords, displaced, level_at}

// HDRP Lit's `_PPDMinSamples`, `_PPDMaxSamples` and `_PPDLodThreshold` defaults.
const MIN_STEPS: f32 = 5.0;
const MAX_STEPS: f32 = 15.0;
const LOD_THRESHOLD: f32 = 5.0;
// The least cosine between the view and the surface the march divides by: a ray
// skimming the surface reads far along it, never infinitely far.
const MIN_COS: f32 = 0.01;
const HALF_PI: f32 = 1.5707963;
const UV0: u32 = 0u;
// The cosines between a face's normal and world Y (Planar's axis) past which the
// shift is kept whole (about 45°) and dropped (about 72°): `projection_fade`.
const PLANAR_FULL: f32 = 0.7;
const PLANAR_NONE: f32 = 0.3;
// How fast the shift may change against the coordinates before its maps leave the
// implicit read (`bend`), and how fast before they are read at the unshifted
// derivatives alone.
const BEND_FROM: f32 = 0.5;
const BEND_TO: f32 = 1.0;

// The height field at `uv`, 1 the surface and 0 the deepest point, at mip `lod`.
fn height_at(uv: vec2<f32>, lod: f32) -> f32 {
    return textureSampleLevel(t_height, s_height, uv, lod).r;
}

// HDRP's `ParallaxOcclusionMapping`: the UV shift where the ray from `uv`, reaching
// `max_offset` at the deepest point, first meets the height field, in `steps` steps
// and one line intersection between the last two.
fn march(uv: vec2<f32>, lod: f32, max_offset: vec2<f32>, steps: i32) -> vec2<f32> {
    let step = 1.0 / f32(steps);
    let per_step = max_offset * step;
    var offset = per_step;
    var prev_height = height_at(uv, lod);
    var curr_height = height_at(uv + offset, lod);
    var ray = 1.0 - step;
    for (var i = 0; i < steps; i++) {
        if (curr_height > ray) {
            break;
        }
        prev_height = curr_height;
        ray -= step;
        offset += per_step;
        curr_height = height_at(uv + offset, lod);
    }
    // The ray and the field cross between the last two steps: where the line
    // between them does.
    let below = curr_height - ray;
    let above = (ray + step) - prev_height;
    return offset - below / max(below + above, 1e-6) * per_step;
}

// How much of the shift a face keeps under its mapping: all of it under UV0;
// under Planar (projected along world Y) all of it within `PLANAR_FULL` of the
// projection axis, none past `PLANAR_NONE`, so a steep face, where the projection
// stretches one repeat across the whole face, draws as Planar without a height map
// does: stretched, never a march along the stretch.
fn projection_fade(mapping: u32, n: vec3<f32>) -> f32 {
    if (mapping == UV0) {
        return 1.0;
    }
    return smoothstep(PLANAR_NONE, PLANAR_FULL, abs(n.y));
}

// `c` (UV0 or Planar, its one lookup in `uv`) shifted by the height map, at world
// `position` with geometric world normal `n` (turned toward the viewer); as given
// for a material without one. Takes derivatives: call it in uniform control flow,
// before any per-pixel branch (the branch here is on `material()`; `shift`'s own
// per-pixel branches take none).
fn displace(c: MapCoords, position: vec3<f32>, n: vec3<f32>) -> MapCoords {
    if (!displaced()) {
        return c;
    }
    let offset = shift(c, position, n, dpdx(position), dpdy(position));
    var out = c;
    out.shift = offset;
    out.uv = c.uv + offset;
    out.x = out.uv;
    out.y = out.uv;
    out.z = out.uv;
    out.bend = bend(offset, c.dx, c.dy);
    return out;
}

// The shift where the march meets the height field from `c`'s one lookup, the
// surface's screen derivatives `dp1` and `dp2` taken by the caller; zero past the
// fade. Takes no derivatives, so its per-pixel branches are safe.
fn shift(c: MapCoords, position: vec3<f32>, n: vec3<f32>, dp1: vec3<f32>, dp2: vec3<f32>) -> vec2<f32> {
    let m = material();
    // The world-to-UV mapping on the surface (Schüler, "Followup: Normal Mapping
    // Without Precomputed Tangents"): `grad_u` and `grad_v` are how fast the tiled
    // UV changes along the surface, in repeats per metre, from the unshifted
    // coordinates' derivatives (`MapCoords`).
    let lod = level_at(vec2<f32>(textureDimensions(t_height)), c.dx, c.dy) + camera.lod.x;
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let det = dot(dp1, dp2perp);
    if (lod >= LOD_THRESHOLD + 1.0 || abs(det) < 1e-20) {
        return vec2<f32>(0.0);
    }
    let grad_u = (dp2perp * c.dx.x + dp1perp * c.dy.x) / det;
    let grad_v = (dp2perp * c.dx.y + dp1perp * c.dy.y) / det;
    // The depth in metres: `height_scale` of one repeat's width, its narrower side.
    // A side the surface does not cross (Planar's projection on a face along Y, a
    // degenerate UV axis) has no width; the narrower one stays finite.
    let width = 1.0 / max(max(length(grad_u), length(grad_v)), 1e-6);
    let depth = m.height_scale * width;
    let v = normalize(camera.camera_pos - position);
    let cos_view = max(dot(v, n), MIN_COS);
    // The view in the height field's own space (HDRP's `viewDirUV`): its lateral
    // part in UV at the deepest point, its normal part the cosine.
    let lateral = depth * vec2<f32>(dot(grad_u, v), dot(grad_v, v));
    let view_uv = normalize(vec3<f32>(lateral, cos_view));
    let unit_angle = saturate(acos(view_uv.z) / HALF_PI);
    let steps = i32(mix(MIN_STEPS, MAX_STEPS, unit_angle));
    // Into the surface, away from the eye.
    let max_offset = -lateral / cos_view;
    // Past the fade (a far surface, a steep Planar face) the march is not paid for;
    // it samples at an explicit level, so this per-pixel branch is safe.
    let fade = (1.0 - saturate(lod - LOD_THRESHOLD)) * projection_fade(m.uv_mapping, n);
    if (fade <= 0.0) {
        return vec2<f32>(0.0);
    }
    return march(c.uv, lod, max_offset, steps) * fade;
}

// How far the shift `offset` bends from pixel to pixel against the unshifted
// coordinates' own derivatives `dx` and `dy`: 0 where it changes under half as fast
// as they do (a smooth shift, a negligible scale, past the fade), rising to 1 where it
// changes as fast (a ridge the ray crosses). A map is read at the shifted
// coordinates' implicit derivatives where this is 0, so a material whose shift is zero
// filters as it does without a height map, and at `dx`/`dy` where it is 1
// (`mapping::lookup_uv`, #2301). Both the coarse and the fine differences are read,
// since a GPU's implicit level may take either. Takes derivatives: call it in uniform
// control flow.
fn bend(offset: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let along_x = max(length(dpdxCoarse(offset)), length(dpdxFine(offset)));
    let along_y = max(length(dpdyCoarse(offset)), length(dpdyFine(offset)));
    let ratio = max(along_x / max(length(dx), 1e-12), along_y / max(length(dy), 1e-12));
    return smoothstep(BEND_FROM, BEND_TO, ratio);
}
