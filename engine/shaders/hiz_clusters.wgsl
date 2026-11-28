// UE5 Nanite's culling half: a mesh split into clusters at import draws, per instance, the
// ones this camera may see. `cs_cluster_test` keeps each (instance, cluster) unless its
// instance is hidden or it is outside the frustum, faces wholly away (normal cone) or is
// behind the pyramid. `cs_cluster_scan` places the kept ones by a prefix sum. The output keeps
// the mesh's own index order, so a frame culling nothing draws exactly the unculled
// triangles.

#import depth
#import hiz::hidden::{hidden, Eye}

struct Params {
    view_proj: mat4x4<f32>,
    // Perspective: the eye's position, `w` 1. Orthographic: the view direction (unit),
    // `w` 0.
    eye: vec4<f32>,
    size: vec2<f32>,
    levels: u32,
    // `ObjectData`'s size in vec4s; the world matrix is its first four.
    object_stride: u32,
    jobs: u32,
    chunks: u32,
};

struct Job {
    instance: u32,
    object: u32,
    tested: u32,
    out_first: u32,
    cluster_first: u32,
    state_first: u32,
    cone: u32,
    clusters: u32,
};

// `min.w` and `max.w` are its first index in the pool and its index count (as bits).
struct Cluster {
    min: vec4<f32>,
    max: vec4<f32>,
    apex: vec4<f32>,
    axis: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> jobs: array<Job>;
@group(0) @binding(2) var<storage, read> objects: array<vec4<f32>>;
@group(0) @binding(3) var<storage, read> visible: array<u32>;
// Word 0 is the kept count; then two words per cluster: the test writes cluster `k`'s verdict
// at `1 + 2k`; the scan writes the `m`-th kept cluster at `1 + 2m`, its output offset at
// `2 + 2m`.
@group(0) @binding(4) var<storage, read_write> state: array<u32>;
@group(0) @binding(5) var<storage, read_write> tally: atomic<u32>;
@group(0) @binding(6) var<storage, read_write> out_args: array<u32>;
@group(0) @binding(7) var pyramid: texture_2d<f32>;
@group(0) @binding(9) var<storage, read_write> out_indices: array<u32>;
@group(0) @binding(10) var<storage, read> chunks: array<vec2<u32>>;
@group(1) @binding(0) var<storage, read> clusters: array<Cluster>;
@group(1) @binding(1) var<storage, read> mesh_indices: array<u32>;

// A dispatch past 65,535 workgroups spills into `y`.
fn flat(group: vec3<u32>, groups: vec3<u32>) -> u32 {
    return group.y * groups.x + group.x;
}

// Cosine past the cone's cutoff a view needs before its cluster faces away.
const CONE_MARGIN: f32 = 1e-3;

fn world_of(object: u32) -> mat4x4<f32> {
    let at = object * params.object_stride;
    return mat4x4<f32>(objects[at], objects[at + 1u], objects[at + 2u], objects[at + 3u]);
}

// Only for a rotation and a uniform scale that doesn't mirror: anything else is kept.
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

fn culled(c: Cluster, world: mat4x4<f32>, cone: bool) -> bool {
    var lo = vec3<f32>(1e30);
    var hi = vec3<f32>(-1e30);
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
        if clip.w <= 1e-5 || depth::before_near(clip) {
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
    workgroupBarrier();
    if lane == 0u {
        let n = atomicLoad(&kept_here);
        if n > 0u {
            atomicAdd(&tally, n);
        }
    }
}

const CHUNK: u32 = 256u;
var<workgroup> sums: array<vec2<u32>, 64>;

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
    var before = vec2<u32>(0u);
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
        var at = before + sums[lane] - own;
        // In place: the chunk's verdicts were read above, and kept cluster `m` has `m <= k`.
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
