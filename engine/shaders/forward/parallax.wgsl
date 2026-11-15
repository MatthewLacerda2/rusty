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

// `c` (UV0 or Planar, its one lookup in `uv`) shifted by the height map, at world
// `position` with geometric world normal `n` (turned toward the viewer); as given
// for a material without one. Takes derivatives: call it in uniform control flow,
// before any per-pixel branch (the branch here is on `material()`).
fn displace(c: MapCoords, position: vec3<f32>, n: vec3<f32>) -> MapCoords {
    if (!displaced()) {
        return c;
    }
    let m = material();
    // The world-to-UV mapping on the surface (Schüler, "Followup: Normal Mapping
    // Without Precomputed Tangents"): `grad_u` and `grad_v` are how fast the tiled
    // UV changes along the surface, in repeats per metre.
    let dp1 = dpdx(position);
    let dp2 = dpdy(position);
    var out = c;
    // Every map is read at these, the unshifted coordinates' (`MapCoords`).
    out.dx = dpdx(c.uv);
    out.dy = dpdy(c.uv);
    let lod = level_at(vec2<f32>(textureDimensions(t_height)), out.dx, out.dy) + camera.lod.x;
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let det = dot(dp1, dp2perp);
    if (lod >= LOD_THRESHOLD + 1.0 || abs(det) < 1e-20) {
        return out;
    }
    let duv1 = out.dx;
    let duv2 = out.dy;
    let grad_u = (dp2perp * duv1.x + dp1perp * duv2.x) / det;
    let grad_v = (dp2perp * duv1.y + dp1perp * duv2.y) / det;
    // The depth in metres: `height_scale` of one repeat's width, the mean of its
    // two sides.
    let width = 0.5 * (1.0 / max(length(grad_u), 1e-6) + 1.0 / max(length(grad_v), 1e-6));
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
    let offset = march(c.uv, lod, max_offset, steps) * (1.0 - saturate(lod - LOD_THRESHOLD));
    out.shift = offset;
    out.uv = c.uv + offset;
    out.x = out.uv;
    out.y = out.uv;
    out.z = out.uv;
    return out;
}
