// Per-cluster occlusion culling (#1968): the culling half of UE5's Nanite. A mesh
// split into clusters at import (`asset::clusters`) is drawn, per instance, as the
// clusters this camera may see, with no mesh shaders. `render::passes::hiz`
// dispatches it per group (the clustered batches of one mesh and cluster
// count), stage by stage, after `cs_test` has its instances' verdicts against
// this frame's pyramid:
//   1. `cs_cluster_test`: each (instance, cluster) is kept unless its instance is
//      hidden, or it is outside the frustum, faces wholly away (its normal cone),
//      or is behind the pyramid (`hidden`). One crossing the near plane is kept;
//   2. `cs_cluster_scan`: a workgroup per instance gives its kept clusters, in
//      order, their place in the output index list by a parallel prefix sum,
//      lists them, and writes the instance's indirect draw and how many
//      clusters the copy dispatches per instance (the most any one kept);
//   3. `cs_cluster_copy`, dispatched indirect from that: a workgroup per kept
//      cluster of each instance copies its indices there. No workgroup starts
//      for a culled cluster but an instance's beyond its own count.
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
};

// One instance of a clustered batch.
struct Job {
    // The instance (in the camera's array), its object slot, and 1 when the
    // per-instance test judged it (else every cluster is drawn).
    instance: u32,
    object: u32,
    tested: u32,
    // Where its output indices start, its first cluster in the mesh's table,
    // its first state word (its region: `1 + 2 * clusters` words), and 1 when
    // its batch keeps front faces only.
    out_first: u32,
    cluster_first: u32,
    state_first: u32,
    cone: u32,
    // Its batch's clusters, the same for every job of its group (`run.clusters`):
    // the plan joins only batches of one count, so no row is padded.
    clusters: u32,
};

// The group a dispatch covers: its jobs, the clusters each of them has, and
// the word of `tally` its copy's indirect dispatch starts at.
struct Run {
    job_first: u32,
    jobs: u32,
    clusters: u32,
    copy_at: u32,
};

// A cluster, local space: `min.w` and `max.w` are its first index and index
// count (as bits); the cone's apex and cutoff, then its axis.
struct Cluster {
    min: vec4<f32>,
    max: vec4<f32>,
    apex: vec4<f32>,
    axis: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> jobs: array<Job>;
@group(0) @binding(2) var<storage, read> objects: array<vec4<f32>>;
// By instance: 1 when `cs_test` drew it this frame.
@group(0) @binding(3) var<storage, read> visible: array<u32>;
// Per job, from `job.state_first`: word 0 its kept clusters' count, written by
// the scan; then two words per cluster. The test writes cluster `k`'s verdict
// at `1 + 2k`; the scan lists its `m`-th kept cluster at `1 + 2m` (which) and
// `2 + 2m` (its output offset), over verdicts it has already read.
@group(0) @binding(4) var<storage, read_write> state: array<u32>;
// Word 0: the tested clusters kept, for `Debug.Stats`; then per group, at its
// `run.copy_at`, the copy's indirect dispatch (most kept clusters, jobs, 1).
@group(0) @binding(5) var<storage, read_write> tally: array<atomic<u32>>;
// One `DrawIndexedIndirectArgs` (five words) per job.
@group(0) @binding(6) var<storage, read_write> out_args: array<u32>;
@group(0) @binding(7) var pyramid: texture_2d<f32>;
@group(0) @binding(8) var<uniform> run: Run;
// The copy's own group 0 binds this beside `jobs`, `state` and `run`, and not
// `tally`, which its dispatch reads as indirect args.
@group(0) @binding(9) var<storage, read_write> out_indices: array<u32>;
@group(1) @binding(0) var<storage, read> clusters: array<Cluster>;
@group(1) @binding(1) var<storage, read> mesh_indices: array<u32>;

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

@compute @workgroup_size(64)
fn cs_cluster_test(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let t = id.x;
    let job = jobs[run.job_first + min(t / run.clusters, run.jobs - 1u)];
    let k = t % run.clusters;
    // Past the group's pairs, or past the job's own clusters: nothing to judge.
    if t < run.jobs * run.clusters && k < job.clusters {
        var keep = true;
        if job.tested != 0u {
            let c = clusters[job.cluster_first + k];
            keep = visible[job.instance] != 0u && !culled(c, world_of(job.object), job.cone != 0u);
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
            atomicAdd(&tally[0], n);
        }
    }
}

// Clusters a scan's chunk covers: four a lane.
const CHUNK: u32 = 256u;
// Per lane: its four clusters' (kept, indices) totals, then their running sum.
var<workgroup> sums: array<vec2<u32>, 64>;

// Dispatched (jobs, 1, 1): one workgroup an instance, its clusters a chunk at a
// time, each chunk's places an inclusive scan over the lanes' totals.
@compute @workgroup_size(64)
fn cs_cluster_scan(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let j = run.job_first + group.x;
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
        atomicMax(&tally[run.copy_at], before.x);
    }
}

// Dispatched indirect (the most kept clusters of an instance, jobs, 1): a
// workgroup per kept cluster, its lanes over the cluster's indices.
@compute @workgroup_size(64)
fn cs_cluster_copy(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let job = jobs[run.job_first + group.y];
    let base = job.state_first;
    if group.x >= state[base] {
        return;
    }
    let k = state[base + 1u + 2u * group.x];
    let at = state[base + 2u + 2u * group.x];
    let c = clusters[job.cluster_first + k];
    let first = bitcast<u32>(c.min.w);
    let count = bitcast<u32>(c.max.w);
    for (var i = lane; i < count; i += 64u) {
        out_indices[at + i] = mesh_indices[first + i];
    }
}
