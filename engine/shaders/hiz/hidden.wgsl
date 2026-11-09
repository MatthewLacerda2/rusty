#define_import_path hiz::hidden

// The conservative Hi-Z box test (#1963), shared by the per-instance test
// (`hiz.wgsl`) and the per-cluster one (`hiz_clusters.wgsl`, #1968). The depth is
// ordinary Z (cleared to 1, compared `Less`) and the pyramid keeps the farthest
// depth under each texel: level `l` texel `t` covers depth pixels `t << (l + 1)`
// up to the next texel's.

// What a test reads of the camera: its view-projection, the depth target's size
// in pixels and the pyramid's level count.
struct Eye {
    view_proj: mat4x4<f32>,
    size: vec2<f32>,
    levels: u32,
};

// Whether the box `lo..hi` is behind the pyramid everywhere it covers.
fn hidden(eye: Eye, pyramid: texture_2d<f32>, lo: vec3<f32>, hi: vec3<f32>) -> bool {
    var rect_min = vec2<f32>(1e30);
    var rect_max = vec2<f32>(-1e30);
    var nearest = 1.0;
    for (var c = 0u; c < 8u; c++) {
        let corner = vec3<f32>(
            select(lo.x, hi.x, (c & 1u) != 0u),
            select(lo.y, hi.y, (c & 2u) != 0u),
            select(lo.z, hi.z, (c & 4u) != 0u),
        );
        let clip = eye.view_proj * vec4<f32>(corner, 1.0);
        // A corner on or behind the near plane: never culled.
        if clip.w <= 1e-5 || clip.z < 0.0 {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        let px = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * eye.size;
        rect_min = min(rect_min, px);
        rect_max = max(rect_max, px);
        nearest = min(nearest, ndc.z);
    }
    let last = eye.size - vec2<f32>(1.0);
    // Wholly off screen: the frustum cull's call, not this one's.
    if any(rect_max < vec2<f32>(0.0)) || any(rect_min > last + vec2<f32>(1.0)) {
        return false;
    }
    let p0 = vec2<u32>(clamp(floor(rect_min), vec2<f32>(0.0), last));
    let p1 = vec2<u32>(clamp(floor(rect_max), vec2<f32>(0.0), last));
    // The level where the rect spans at most 2x2 texels: level `l` texel `t` covers
    // pixels `t << (l + 1)` up to the next texel's.
    var level = 0u;
    loop {
        let t0 = p0 >> vec2<u32>(level + 1u);
        let t1 = p1 >> vec2<u32>(level + 1u);
        if all(t1 - t0 <= vec2<u32>(1u)) || level + 1u >= eye.levels {
            break;
        }
        level++;
    }
    // The level's size from level 0's, and its texels read in a loop over every
    // level: `textureDimensions` and `textureLoad` at a level that differs per
    // thread answered another thread's on lavapipe, so each call's level is the
    // loop's, the same in every thread.
    let base = textureDimensions(pyramid, 0u);
    var farthest = 0.0;
    for (var l = 0u; l < eye.levels; l++) {
        if l != level {
            continue;
        }
        let dims = max(base >> vec2<u32>(l), vec2<u32>(1u)) - vec2<u32>(1u);
        let t0 = min(p0 >> vec2<u32>(l + 1u), dims);
        let t1 = min(p1 >> vec2<u32>(l + 1u), dims);
        for (var y = t0.y; y <= t1.y; y++) {
            for (var x = t0.x; x <= t1.x; x++) {
                farthest = max(farthest, textureLoad(pyramid, vec2<u32>(x, y), l).r);
            }
        }
    }
    // A face on the box's own near plane rasterizes to its corners' depth give or
    // take rounding: hidden only past a margin of float steps (64, about 4e-6 near
    // 1, a centimetre and a half at 20 m from a 0.1 m near plane), never by it.
    return bitcast<u32>(nearest) > bitcast<u32>(farthest) + MARGIN;
}

// The depth margin, in float steps (`hidden`); depths are in `0..1`, so their
// bits order as they do.
const MARGIN: u32 = 64u;
