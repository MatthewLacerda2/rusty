// Per-cluster occlusion culling (#1968): the culling half of UE5's Nanite. A mesh
// split into clusters at import (`asset::clusters`) is drawn, per instance, as the
// clusters this camera may see, with no mesh shaders. `render::passes::hiz`
// dispatches each stage once for the whole frame (#2417): every clustered mesh's
// table and indices are in one pool (group 1), so one dispatch reaches every
// job. After `cs_test` has its instances' verdicts against this frame's pyramid:
//   1. `cs_cluster_test`, a workgroup per chunk (64 clusters of one job): each
//      (instance, cluster) is kept unless its instance is hidden, or it is
//      outside the frustum, faces wholly away (its normal cone), or is behind
//      the pyramid (`hidden`). One crossing the near plane is kept;
//   2. `cs_cluster_scan`, a workgroup per job: gives its kept clusters, in
//      order, their place in the output index list by a parallel prefix sum,
//      lists them, and writes the instance's indirect draw;
//   3. `cs_cluster_copy`, a workgroup per chunk again: copies the kept clusters
//      listed in its 64 places of the job's list, returning at once when the
//      job kept fewer than its first.
// The output keeps the clusters' order, which is the mesh's own reordered index
// order, so a frame that culls nothing draws exactly the uncull triangles, and
// every place is a prefix sum: two frames of one view write one index stream.

#import hiz::hidden::{hidden, Eye}

struct Params {
    view_proj: mat4x4<f32>,
    // Perspective: the eye's position, `w` 1. Orthographic: the view direction
    // (unit), `w` 0.
    eye: vec4<f32>,
    size: vec2<f32>,
    levels: u32,
    // `ObjectData`'s size in `vec4`s: the world matrix is the first four.
    object_stride: u32,
    // The frame's jobs and chunks: the scan's and the other two's workgroups.
    jobs: u32,
    chunks: u32,
};

// One instance of a clustered batch.
struct Job {
    // The instance (in the camera's array), its object slot, and 1 when the
    // per-instance test judged it (else every cluster is drawn).
    instance: u32,
    object: u32,
    tested: u32,
    // Where its output indices start, its first cluster in the shared table,
    // its first state word (its region: `1 + 2 * clusters` words), and 1 when
    // its batch keeps front faces only.
    out_first: u32,
    cluster_first: u32,
    state_first: u32,
    cone: u32,
    // Its batch's clusters.
    clusters: u32,
};

// A cluster, local space: `min.w` and `max.w` are its first index in the pool
// and its index count (as bits); the cone's apex and cutoff, then its axis.
struct Cluster {
    min: vec4<f32>,
    max: vec4<f32>,
    apex: vec4<f32>,
    axis: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> jobs: array<Job>;
@group(0) @binding(2) var<storage, read> objects: array<vec4<f32>>;
// By instance: bit 0 when `cs_test` drew it this frame (bit 1: phase one did).
@group(0) @binding(3) var<storage, read> visible: array<u32>;
// Per job, from `job.state_first`: word 0 its kept clusters' count, written by
// the scan; then two words per cluster. The test writes cluster `k`'s verdict
// at `1 + 2k`; the scan lists its `m`-th kept cluster at `1 + 2m` (which) and
// `2 + 2m` (its output offset), over verdicts it has already read.
@group(0) @binding(4) var<storage, read_write> state: array<u32>;
// The tested clusters kept, for `Debug.Stats`.
@group(0) @binding(5) var<storage, read_write> tally: atomic<u32>;
// One `DrawIndexedIndirectArgs` (five words) per job.
@group(0) @binding(6) var<storage, read_write> out_args: array<u32>;
@group(0) @binding(7) var pyramid: texture_2d<f32>;
@group(0) @binding(9) var<storage, read_write> out_indices: array<u32>;
// Per chunk: its job and the job's first cluster it covers. Each stage's own
// group 0 binds only what it reads (`clusters::layout`).
@group(0) @binding(10) var<storage, read> chunks: array<vec2<u32>>;
// The pool: every clustered mesh's rows, and a copy of its indices.
@group(1) @binding(0) var<storage, read> clusters: array<Cluster>;
@group(1) @binding(1) var<storage, read> mesh_indices: array<u32>;

// A workgroup's place in a dispatch that may take a second dimension past
// 65,535 workgroups (`clusters::record::workgroups`).
fn flat(group: vec3<u32>, groups: vec3<u32>) -> u32 {
    return group.y * groups.x + group.x;
}

// How far past its cutoff a cone must face away before it is culled.
const CONE_MARGIN: f32 = 1e-3;

fn world_of(object: u32) -> mat4x4<f32> {
    let at = object * params.object_stride;
    return mat4x4<f32>(objects[at], objects[at + 1u], objects[at + 2u], objects[at + 3u]);
}

// Whether every triangle of `c`, placed by `world`, faces away from the eye. Only
// for a rotation and a uniform scale that doesn't mirror: anything else is kept.
fn faces_away(c: Cluster, world: mat4x4<f32>) -> bool {
    if c.apex.w >= 1.0 {
        return false;
    }
    let m = mat3x3<f32>(world[0].xyz, world[1].xyz, world[2].xyz);
    let l = vec3<f32>(length(m[0]), length(m[1]), length(m[2]));
    let skew = abs(vec3<f32>(dot(m[0], m[1]), dot(m[1], m[2]), dot(m[0], m[2])));
    let uniform_scale = max(l.x, max(l.y, l.z)) <= min(l.x, min(l.y, l.z)) * 1.001;
    if determinant(m) <= 0.0 || !uniform_scale || any(skew > vec3<f32>(1e-3 * l.x * l.x)) {
        return false;
    }
    let axis = normalize(m * c.axis.xyz);
    var view = params.eye.xyz;
    if params.eye.w > 0.5 {
        let apex = (world * vec4<f32>(c.apex.xyz, 1.0)).xyz;
        view = normalize(apex - params.eye.xyz);
    }
    return dot(view, axis) >= c.apex.w + CONE_MARGIN;
}

// Whether the cluster `c` of a visible instance placed by `world` is culled.
fn culled(c: Cluster, world: mat4x4<f32>, cone: bool) -> bool {
    var lo = vec3<f32>(1e30);
    var hi = vec3<f32>(-1e30);
    // Per frustum plane, whether every corner is outside it.
    var out_lo = vec3<bool>(true);
    var out_hi = vec3<bool>(true);
    for (var k = 0u; k < 8u; k++) {
        let local = vec3<f32>(
            select(c.min.x, c.max.x, (k & 1u) != 0u),
            select(c.min.y, c.max.y, (k & 2u) != 0u),
            select(c.min.z, c.max.z, (k & 4u) != 0u),
        );
        let p = (world * vec4<f32>(local, 1.0)).xyz;
        lo = min(lo, p);
        hi = max(hi, p);
        let clip = params.view_proj * vec4<f32>(p, 1.0);
        // A corner on or behind the near plane: never culled.
        if clip.w <= 1e-5 || clip.z < 0.0 {
            return false;
        }
        out_lo = out_lo & (clip.xyz < vec3<f32>(-clip.w, -clip.w, 0.0));
        out_hi = out_hi & (clip.xyz > vec3<f32>(clip.w));
    }
    if any(out_lo) || any(out_hi) {
        return true;
    }
    if cone && faces_away(c, world) {
        return true;
    }
    return hidden(Eye(params.view_proj, params.size, params.levels), pyramid, lo, hi);
}

var<workgroup> kept_here: atomic<u32>;

// A workgroup per chunk, a lane per cluster of it.
@compute @workgroup_size(64)
fn cs_cluster_test(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let w = flat(group, groups);
    // The last row's overhang: the whole workgroup leaves, before any barrier.
    if w >= params.chunks {
        return;
    }
    let chunk = chunks[w];
    let job = jobs[chunk.x];
    let k = chunk.y + lane;
    if k < job.clusters {
        var keep = true;
        if job.tested != 0u {
            let c = clusters[job.cluster_first + k];
            keep = (visible[job.instance] & 1u) != 0u && !culled(c, world_of(job.object), job.cone != 0u);
            if keep {
                atomicAdd(&kept_here, 1u);
            }
        }
        state[job.state_first + 1u + 2u * k] = u32(keep);
    }
    // One add to the count a workgroup, not one a kept cluster.
    workgroupBarrier();
    if lane == 0u {
        let n = atomicLoad(&kept_here);
        if n > 0u {
            atomicAdd(&tally, n);
        }
    }
}

// Clusters a scan's chunk covers: four a lane.
const CHUNK: u32 = 256u;
// Per lane: its four clusters' (kept, indices) totals, then their running sum.
var<workgroup> sums: array<vec2<u32>, 64>;

// One workgroup a job, its clusters 256 at a time, each such run's places an
// inclusive scan over the lanes' totals.
@compute @workgroup_size(64)
fn cs_cluster_scan(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let j = flat(group, groups);
    if j >= params.jobs {
        return;
    }
    let job = jobs[j];
    let base = job.state_first;
    // Kept clusters and their indices before this chunk.
    var before = vec2<u32>(0u);
    // Uniform across the workgroup: every lane reads the same job.
    for (var chunk = 0u; chunk < job.clusters; chunk += CHUNK) {
        var keep = vec4<u32>(0u);
        var count = vec4<u32>(0u);
        for (var i = 0u; i < 4u; i++) {
            let k = chunk + lane * 4u + i;
            if k < job.clusters && state[base + 1u + 2u * k] != 0u {
                keep[i] = 1u;
                count[i] = bitcast<u32>(clusters[job.cluster_first + k].max.w);
            }
        }
        let own = vec2<u32>(keep.x + keep.y + keep.z + keep.w, count.x + count.y + count.z + count.w);
        sums[lane] = own;
        workgroupBarrier();
        for (var d = 1u; d < 64u; d <<= 1u) {
            var v = sums[lane];
            if lane >= d {
                v += sums[lane - d];
            }
            workgroupBarrier();
            sums[lane] = v;
            workgroupBarrier();
        }
        // Every verdict of the chunk was read above, and a kept cluster is listed
        // at or before its own place: the writes overwrite only what was read.
        var at = before + sums[lane] - own;
        for (var i = 0u; i < 4u; i++) {
            if keep[i] != 0u {
                state[base + 1u + 2u * at.x] = chunk + lane * 4u + i;
                state[base + 2u + 2u * at.x] = job.out_first + at.y;
                at += vec2<u32>(1u, count[i]);
            }
        }
        before += sums[63];
        workgroupBarrier();
    }
    if lane == 0u {
        state[base] = before.x;
        let args = j * 5u;
        out_args[args] = before.y;
        out_args[args + 1u] = u32(before.y > 0u);
        out_args[args + 2u] = job.out_first;
        out_args[args + 3u] = 0u;
        out_args[args + 4u] = job.instance;
    }
}

// A workgroup per chunk: the job's kept clusters listed at the chunk's 64
// places (from its first cluster), each copied by all 64 lanes. A chunk past
// what its job kept returns at once.
@compute @workgroup_size(64)
fn cs_cluster_copy(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(num_workgroups) groups: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let w = flat(group, groups);
    if w >= params.chunks {
        return;
    }
    let chunk = chunks[w];
    let job = jobs[chunk.x];
    let base = job.state_first;
    let end = min(chunk.y + 64u, state[base]);
    for (var m = chunk.y; m < end; m++) {
        let k = state[base + 1u + 2u * m];
        let at = state[base + 2u + 2u * m];
        let c = clusters[job.cluster_first + k];
        let first = bitcast<u32>(c.min.w);
        let count = bitcast<u32>(c.max.w);
        for (var i = lane; i < count; i += 64u) {
            out_indices[at + i] = mesh_indices[first + i];
        }
    }
}
