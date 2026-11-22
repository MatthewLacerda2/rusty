// HDRP Lit's *Displacement Mode: Pixel* (parallax occlusion mapping): `displace` shifts
// the one `MapCoords` every map reads. The depth the surface writes is untouched, so
// outline, shadows, decals, SSR, SSAO and motion vectors see the flat surface. The march is
// HDRP's `ParallaxOcclusionMapping` (`PerPixelDisplacement.hlsl`): a linear search, then one
// line intersection.

#define_import_path forward::parallax

#import forward::bindings::{camera, material, s_height, t_height}
#import forward::mapping::{MapCoords, displaced, level_at}

// HDRP Lit's `_PPDMinSamples`, `_PPDMaxSamples` and `_PPDLodThreshold` (a mip) defaults.
const MIN_STEPS: f32 = 5.0;
const MAX_STEPS: f32 = 15.0;
const LOD_THRESHOLD: f32 = 5.0;
// The least view cosine the march divides by, so a ray skimming the surface reads far along
// it, never infinitely far.
const MIN_COS: f32 = 0.01;
const HALF_PI: f32 = 1.5707963;
const UV0: u32 = 0u;
// Cosines of a face's normal with world Y (Planar's axis) above which the shift is whole, and
// below which it is none.
const PLANAR_FULL: f32 = 0.7;
const PLANAR_NONE: f32 = 0.3;
const BEND_FROM: f32 = 0.5;
const BEND_TO: f32 = 1.0;

fn height_at(uv: vec2<f32>, lod: f32) -> f32 {
    return textureSampleLevel(t_height, s_height, uv, lod).r;
}

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
    let below = curr_height - ray;
    let above = (ray + step) - prev_height;
    return offset - below / max(below + above, 1e-6) * per_step;
}

fn projection_fade(mapping: u32, n: vec3<f32>) -> f32 {
    if (mapping == UV0) {
        return 1.0;
    }
    return smoothstep(PLANAR_NONE, PLANAR_FULL, abs(n.y));
}

// Takes derivatives: call it in uniform control flow, before any per-pixel branch.
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

// Takes no derivatives, so its per-pixel branches are safe.
fn shift(c: MapCoords, position: vec3<f32>, n: vec3<f32>, dp1: vec3<f32>, dp2: vec3<f32>) -> vec2<f32> {
    let m = material();
    // Schüler, "Followup: Normal Mapping Without Precomputed Tangents"; `grad_u` and
    // `grad_v` are in repeats per metre.
    let lod = level_at(vec2<f32>(textureDimensions(t_height)), c.dx, c.dy) + camera.lod.x;
    let dp2perp = cross(dp2, n);
    let dp1perp = cross(n, dp1);
    let det = dot(dp1, dp2perp);
    if (lod >= LOD_THRESHOLD + 1.0 || abs(det) < 1e-20) {
        return vec2<f32>(0.0);
    }
    let grad_u = (dp2perp * c.dx.x + dp1perp * c.dy.x) / det;
    let grad_v = (dp2perp * c.dx.y + dp1perp * c.dy.y) / det;
    // One repeat's narrower side in metres (finite where the surface does not cross the other);
    // the depth is `height_scale` of it.
    let width = 1.0 / max(max(length(grad_u), length(grad_v)), 1e-6);
    let depth = m.height_scale * width;
    let v = normalize(camera.camera_pos - position);
    let cos_view = max(dot(v, n), MIN_COS);
    let lateral = depth * vec2<f32>(dot(grad_u, v), dot(grad_v, v));
    let view_uv = normalize(vec3<f32>(lateral, cos_view));
    let unit_angle = saturate(acos(view_uv.z) / HALF_PI);
    let steps = i32(mix(MIN_STEPS, MAX_STEPS, unit_angle));
    let max_offset = -lateral / cos_view;
    let fade = (1.0 - saturate(lod - LOD_THRESHOLD)) * projection_fade(m.uv_mapping, n);
    if (fade <= 0.0) {
        return vec2<f32>(0.0);
    }
    return march(c.uv, lod, max_offset, steps) * fade;
}

// 0 where the shift changes under half as fast as the derivatives, 1 where as fast (a
// ridge the ray crosses). Coarse and fine both, since a GPU's implicit level may take either.
// Takes derivatives: call it in uniform control flow.
fn bend(offset: vec2<f32>, dx: vec2<f32>, dy: vec2<f32>) -> f32 {
    let along_x = max(length(dpdxCoarse(offset)), length(dpdxFine(offset)));
    let along_y = max(length(dpdyCoarse(offset)), length(dpdyFine(offset)));
    let ratio = max(along_x / max(length(dx), 1e-12), along_y / max(length(dy), 1e-12));
    return smoothstep(BEND_FROM, BEND_TO, ratio);
}
