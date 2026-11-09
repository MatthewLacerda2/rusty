// GPU occlusion culling (#1963): Unity 6's GPU occlusion culling, two-phase Hi-Z
// per instance. `render::hiz` drives it, once per camera:
//   1. `cs_select`: each opaque batch keeps the instances visible last frame (by
//      object slot, `history`), in order, and writes its indirect args;
//   2. the prepass draws those into the view's depth;
//   3. `hiz_pyramid.wgsl`: a max-reduced pyramid of it;
//   4. `cs_test`: each instance's world bounds against the pyramid, into
//      `visible` and next frame's `history`;
//   5. `cs_compact`: each batch keeps its visible instances, in order, and writes
//      the indirect args every opaque pass then draws with.
// The depth is ordinary Z (cleared to 1, compared `Less`), so the pyramid keeps the
// farthest depth under each texel, and an instance is hidden only when its nearest
// point is behind it.

struct Params {
    view_proj: mat4x4<f32>,
    // The depth target's size in pixels, the pyramid's level count, then how many
    // instances and batches this camera tests.
    size: vec2<f32>,
    levels: u32,
    instances: u32,
    batches: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

// An instance's world bounds; `min.w` is 1 when it is tested at all (its bounds
// are known and nothing keeps it drawn), else it is always drawn.
struct Bounds {
    min: vec4<f32>,
    max: vec4<f32>,
};

// One opaque batch: its instance run and the indexed draw it makes.
struct Batch {
    first: u32,
    count: u32,
    index_count: u32,
    first_index: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> bounds: array<Bounds>;
@group(0) @binding(2) var<storage, read> batches: array<Batch>;
// The camera's packed instances (`InstanceData`, 16 bytes; `.x` is the object slot).
@group(0) @binding(3) var<storage, read> instances: array<vec4<u32>>;
// By object slot: 1 when it was drawn last frame.
@group(0) @binding(4) var<storage, read_write> history: array<u32>;
// By instance: 1 when this frame draws it.
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
// The kept instances at their batch's run, and one `DrawIndexedIndirectArgs`
// (five words) per batch.
@group(0) @binding(6) var<storage, read_write> kept: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read_write> args: array<u32>;
@group(0) @binding(8) var pyramid: texture_2d<f32>;

// Keep batch `b`'s instances `keep` admits, in order, and write its draw.
fn compact(b: u32, phase_one: bool) {
    let batch = batches[b];
    var n = 0u;
    for (var k = 0u; k < batch.count; k++) {
        let i = batch.first + k;
        let instance = instances[i];
        var keep: bool;
        if phase_one {
            keep = bounds[i].min.w < 0.5 || history[instance.x] != 0u;
        } else {
            keep = visible[i] != 0u;
        }
        if keep {
            kept[batch.first + n] = instance;
            n++;
        }
    }
    let at = b * 5u;
    args[at] = batch.index_count;
    args[at + 1u] = n;
    args[at + 2u] = batch.first_index;
    args[at + 3u] = 0u;
    args[at + 4u] = batch.first;
}

@compute @workgroup_size(64)
fn cs_select(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < params.batches {
        compact(id.x, true);
    }
}

@compute @workgroup_size(64)
fn cs_compact(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < params.batches {
        compact(id.x, false);
    }
}

// Whether the box `lo..hi` is behind the pyramid everywhere it covers.
fn hidden(lo: vec3<f32>, hi: vec3<f32>) -> bool {
    var rect_min = vec2<f32>(1e30);
    var rect_max = vec2<f32>(-1e30);
    var nearest = 1.0;
    for (var c = 0u; c < 8u; c++) {
        let corner = vec3<f32>(
            select(lo.x, hi.x, (c & 1u) != 0u),
            select(lo.y, hi.y, (c & 2u) != 0u),
            select(lo.z, hi.z, (c & 4u) != 0u),
        );
        let clip = params.view_proj * vec4<f32>(corner, 1.0);
        // A corner on or behind the near plane: never culled.
        if clip.w <= 1e-5 || clip.z < 0.0 {
            return false;
        }
        let ndc = clip.xyz / clip.w;
        let px = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5) * params.size;
        rect_min = min(rect_min, px);
        rect_max = max(rect_max, px);
        nearest = min(nearest, ndc.z);
    }
    let last = params.size - vec2<f32>(1.0);
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
        if all(t1 - t0 <= vec2<u32>(1u)) || level + 1u >= params.levels {
            break;
        }
        level++;
    }
    let dims = textureDimensions(pyramid, level) - vec2<u32>(1u);
    let t0 = min(p0 >> vec2<u32>(level + 1u), dims);
    let t1 = min(p1 >> vec2<u32>(level + 1u), dims);
    var farthest = 0.0;
    for (var y = t0.y; y <= t1.y; y++) {
        for (var x = t0.x; x <= t1.x; x++) {
            farthest = max(farthest, textureLoad(pyramid, vec2<u32>(x, y), level).r);
        }
    }
    return nearest > farthest;
}

@compute @workgroup_size(64)
fn cs_test(@builtin(global_invocation_id) id: vec3<u32>) {
    let i = id.x;
    if i >= params.instances {
        return;
    }
    let b = bounds[i];
    var drawn = true;
    if b.min.w > 0.5 {
        drawn = !hidden(b.min.xyz, b.max.xyz);
    }
    visible[i] = u32(drawn);
    history[instances[i].x] = u32(drawn);
}
