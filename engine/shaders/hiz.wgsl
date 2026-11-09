// GPU occlusion culling (#1963): Unity 6's GPU occlusion culling, two-phase Hi-Z
// per instance. `render::hiz` drives it, once per camera:
//   1. `cs_select`: each opaque batch keeps the instances visible last frame (by
//      object slot, `history`), in order, and writes its indirect args;
//   2. the prepass draws those into the view's depth;
//   3. `hiz_pyramid.wgsl`: a max-reduced pyramid of it;
//   4. `cs_test`: each instance's world bounds against the pyramid, into
//      `visible` and next frame's `history`;
//   5. `cs_compact`: each batch keeps its visible instances, in order, and writes
//      the indirect args every opaque pass then draws with. A batch whose mesh is
//      split into clusters (#1968) keeps every instance at its own place instead:
//      `hiz_clusters.wgsl` then draws each instance's visible clusters on its own.
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
// `clustered` when its final draws are per instance, by cluster (#1968).
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
// By instance: 1 when this frame draws it.
@group(0) @binding(5) var<storage, read_write> visible: array<u32>;
// The kept instances at their batch's run, and one `DrawIndexedIndirectArgs`
// (five words) per batch.
@group(0) @binding(6) var<storage, read_write> kept: array<vec4<u32>>;
@group(0) @binding(7) var<storage, read_write> args: array<u32>;
@group(0) @binding(8) var pyramid: texture_2d<f32>;

// Keep batch `b`'s instances `keep` admits, in order, and write its draw. A
// clustered batch's final pass keeps every instance at its own place (its draws
// are per instance), counting the visible ones.
fn compact(b: u32, phase_one: bool) {
    let batch = batches[b];
    let in_place = !phase_one && batch.clustered != 0u;
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
        if in_place {
            kept[i] = instance;
        } else if keep {
            kept[batch.first + n] = instance;
        }
        if keep {
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
    visible[i] = u32(drawn);
    history[instances[i].x] = u32(drawn);
}
