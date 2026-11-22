// Unity 6's GPU occlusion culling, two-phase Hi-Z per instance, run once per camera by
// `render::hiz`: `cs_select` keeps the instances visible last frame (`history`, by
// object slot) and writes indirect args; phase one draws them into the view's depth;
// `hiz_pyramid.wgsl` max-reduces it; `cs_test` tests each instance's bounds against the
// pyramid into `visible` and next frame's `history`; `cs_compact` keeps the visible
// ones and writes the args every opaque pass draws with. The late set is the visible
// instances phase one did not draw. The depth is ordinary Z (cleared to 1, `Less`), so
// the pyramid keeps the farthest depth under each texel.

#import hiz::hidden::{hidden, Eye}

struct Params {
    view_proj: mat4x4<f32>,
    size: vec2<f32>,
    levels: u32,
    instances: u32,
    batches: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

// `min.w` is 1 when the instance is tested at all, else it is always drawn.
struct Bounds {
    min: vec4<f32>,
    max: vec4<f32>,
};

struct Batch {
    first: u32,
    count: u32,
    index_count: u32,
    first_index: u32,
    clustered: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> bounds: array<Bounds>;
@group(0) @binding(2) var<storage, read> batches: array<Batch>;
@group(0) @binding(3) var<storage, read> instances: array<vec4<u32>>;
@group(0) @binding(4) var<storage, read_write> history: array<u32>;
// Bit 0: this frame draws it (`cs_test`); bit 1: phase one drew it (`cs_select`).
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
@group(0) @binding(6) var<storage, read_write> kept: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read_write> args: array<u32>;
@group(0) @binding(8) var pyramid: texture_2d<f32>;

fn write_args(at: u32, b: u32, n: u32, first: u32) {
    let batch = batches[b];
    args[at] = batch.index_count;
    args[at + 1u] = n;
    args[at + 2u] = batch.first_index;
    args[at + 3u] = 0u;
    args[at + 4u] = first;
}

fn keep_early(b: u32) {
    let batch = batches[b];
    var n = 0u;
    for (var k = 0u; k < batch.count; k++) {
        let i = batch.first + k;
        let instance = instances[i];
        let keep = bounds[i].min.w < 0.5 || history[instance.x] != 0u;
        visible[i] = select(0u, 2u, keep);
        if keep {
            kept[batch.first + n] = instance;
            n++;
        }
    }
    write_args(b * 5u, b, n, batch.first);
}

fn compact(b: u32) {
    let batch = batches[b];
    let in_place = batch.clustered != 0u;
    let late_first = params.instances + batch.first;
    var n = 0u;
    var late = 0u;
    for (var k = 0u; k < batch.count; k++) {
        let i = batch.first + k;
        let instance = instances[i];
        let flags = visible[i];
        let keep = (flags & 1u) != 0u;
        if in_place {
            kept[i] = instance;
        } else if keep {
            kept[batch.first + n] = instance;
        }
        if keep {
            n++;
        }
        if keep && (flags & 2u) == 0u {
            kept[late_first + late] = instance;
            late++;
        }
    }
    write_args(b * 5u, b, n, batch.first);
    write_args((params.batches + b) * 5u, b, late, late_first);
}

@compute @workgroup_size(64)
fn cs_select(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < params.batches {
        keep_early(id.x);
    }
}

@compute @workgroup_size(64)
fn cs_compact(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x < params.batches {
        compact(id.x);
    }
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
        let eye = Eye(params.view_proj, params.size, params.levels);
        drawn = !hidden(eye, pyramid, b.min.xyz, b.max.xyz);
    }
    visible[i] = (visible[i] & 2u) | u32(drawn);
    history[instances[i].x] = u32(drawn);
}
