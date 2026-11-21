// GPU occlusion culling: Unity 6's GPU occlusion culling, two-phase Hi-Z
// per instance. `render::hiz` drives it, once per camera:
//   1. `cs_select`: each opaque batch keeps the instances visible last frame (by
//      object slot, `history`), in order, and writes its indirect args, marking
//      each it keeps as drawn early (`visible`'s bit 1);
//   2. phase one draws those into the view's depth, which the prepass keeps;
//   3. `hiz_pyramid.wgsl`: a max-reduced pyramid of it;
//   4. `cs_test`: each instance's world bounds against the pyramid, into
//      `visible` and next frame's `history`;
//   5. `cs_compact`: each batch keeps its visible instances, in order, and writes
//      the indirect args every opaque pass then draws with. A batch whose mesh is
//      split into clusters keeps every instance at its own place instead:
//      `hiz_clusters.wgsl` then draws each instance's visible clusters on its own.
//      The same loop writes the late set: the visible instances phase one
//      did not draw, which is all the prepass adds to phase one's depth. Its kept
//      instances follow the final ones (from `params.instances`), its args the
//      final args (from batch `params.batches`); a clustered batch's late
//      instances are drawn whole, whose depth equals its kept clusters'.
// The depth is ordinary Z (cleared to 1, compared `Less`), so the pyramid keeps the
// farthest depth under each texel, and an instance is hidden only when its nearest
// point is behind it.

#import hiz::hidden::{hidden, Eye}

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

// One opaque batch: its instance run and the indexed draw it makes, and 1 in
// `clustered` when its final draws are per instance, by cluster.
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
// The camera's packed instances (`InstanceData`, 16 bytes; `.x` is the object slot).
@group(0) @binding(3) var<storage, read> instances: array<vec4<u32>>;
// By object slot: 1 when it was drawn last frame.
@group(0) @binding(4) var<storage, read_write> history: array<u32>;
// By instance: bit 0 when this frame draws it (`cs_test`), bit 1 when phase one
// drew it (`cs_select`).
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
// The kept instances at their batch's run, and one `DrawIndexedIndirectArgs`
// (five words) per batch; the final group's hold the late set's after them.
@group(0) @binding(6) var<storage, read_write> kept: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read_write> args: array<u32>;
@group(0) @binding(8) var pyramid: texture_2d<f32>;

// Write batch `b`'s draw of `n` instances from `first`, at args `at`.
fn write_args(at: u32, b: u32, n: u32, first: u32) {
    let batch = batches[b];
    args[at] = batch.index_count;
    args[at + 1u] = n;
    args[at + 2u] = batch.first_index;
    args[at + 3u] = 0u;
    args[at + 4u] = first;
}

// Keep batch `b`'s instances phase one draws (always-drawn, or drawn last frame),
// in order, mark them drawn early, and write its draw.
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

// Keep batch `b`'s visible instances, in order, and write its draw; and its late
// set, the visible ones phase one did not draw, after the final lists. A
// clustered batch's final pass keeps every instance at its own place (its draws
// are per instance), counting the visible ones.
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
