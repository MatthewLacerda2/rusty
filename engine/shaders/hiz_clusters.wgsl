// Per-cluster occlusion culling (#1968): the culling half of UE5's Nanite. A mesh
// split into clusters at import (`asset::clusters`) is drawn, per instance, as the
// clusters this camera may see, with no mesh shaders. `render::passes::hiz`
// dispatches it per clustered batch, after `cs_test` has its instances' verdicts
// against this frame's pyramid:
//   1. `cs_cluster_test`: each (instance, cluster) is kept unless its instance is
//      hidden, or it is outside the frustum, faces wholly away (its normal cone),
//      or is behind the pyramid (`hidden`). One crossing the near plane is kept;
//   2. `cs_cluster_scan`: each instance's kept clusters, in order, get their
//      place in the output index list, and the instance its indirect draw;
//   3. `cs_cluster_copy`: a workgroup per (instance, cluster) copies a kept
//      cluster's indices there.
// The output keeps the clusters' order, which is the mesh's own reordered index
// order, so a frame that culls nothing draws exactly the uncull triangles.

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
    // its first state word, and 1 when its batch keeps front faces only.
    out_first: u32,
    cluster_first: u32,
    state_first: u32,
    cone: u32,
    _pad: u32,
};

// The batch a dispatch covers: its jobs, and how many clusters each has.
struct Run {
    job_first: u32,
    jobs: u32,
    clusters: u32,
    _pad: u32,
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
// Word 0: the tested clusters kept, for `Debug.Stats`. Then per (job, cluster),
// from `job.state_first`: 1 or 0 after the test, its output offset or `NONE`
// after the scan. Atomic for the count's sake; the rest is plain loads and stores.
@group(0) @binding(4) var<storage, read_write> state: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> out_indices: array<u32>;
// One `DrawIndexedIndirectArgs` (five words) per job.
@group(0) @binding(6) var<storage, read_write> out_args: array<u32>;
@group(0) @binding(7) var pyramid: texture_2d<f32>;
@group(0) @binding(8) var<uniform> run: Run;
@group(1) @binding(0) var<storage, read> clusters: array<Cluster>;
@group(1) @binding(1) var<storage, read> mesh_indices: array<u32>;

const NONE: u32 = 0xffffffffu;
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

@compute @workgroup_size(64)
fn cs_cluster_test(@builtin(global_invocation_id) id: vec3<u32>) {
    let t = id.x;
    if t >= run.jobs * run.clusters {
        return;
    }
    let job = jobs[run.job_first + t / run.clusters];
    let k = t % run.clusters;
    var keep = true;
    if job.tested != 0u {
        let c = clusters[job.cluster_first + k];
        keep = visible[job.instance] != 0u && !culled(c, world_of(job.object), job.cone != 0u);
        if keep {
            atomicAdd(&state[0], 1u);
        }
    }
    atomicStore(&state[job.state_first + k], u32(keep));
}

@compute @workgroup_size(64)
fn cs_cluster_scan(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= run.jobs {
        return;
    }
    let j = run.job_first + id.x;
    let job = jobs[j];
    var n = 0u;
    for (var k = 0u; k < run.clusters; k++) {
        let at = job.state_first + k;
        if atomicLoad(&state[at]) != 0u {
            atomicStore(&state[at], job.out_first + n);
            n += bitcast<u32>(clusters[job.cluster_first + k].max.w);
        } else {
            atomicStore(&state[at], NONE);
        }
    }
    let args = j * 5u;
    out_args[args] = n;
    out_args[args + 1u] = u32(n > 0u);
    out_args[args + 2u] = job.out_first;
    out_args[args + 3u] = 0u;
    out_args[args + 4u] = job.instance;
}

// Dispatched (clusters, jobs): one workgroup per (instance, cluster).
@compute @workgroup_size(64)
fn cs_cluster_copy(
    @builtin(workgroup_id) group: vec3<u32>,
    @builtin(local_invocation_index) lane: u32,
) {
    let job = jobs[run.job_first + group.y];
    let at = atomicLoad(&state[job.state_first + group.x]);
    if at == NONE {
        return;
    }
    let c = clusters[job.cluster_first + group.x];
    let first = bitcast<u32>(c.min.w);
    let count = bitcast<u32>(c.max.w);
    for (var i = lane; i < count; i += 64u) {
        out_indices[at + i] = mesh_indices[first + i];
    }
}
