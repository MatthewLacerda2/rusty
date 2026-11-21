// Unity HDRP's *Automatic Histogram*. The adapted EV stays on the GPU (`state.x`),
// where the composite reads it: no readback. Integer atomics and a serial reduce keep
// it a pure function of the scene and the params, so a capture replays exactly.

#import postfx::grade::luminance

const BINS: u32 = 64u;
// Every pixel is metered, so a lamp a few pixels wide counts by its area wherever it
// falls.
const BLOCK: u32 = 4u;
// HDRP's *Center Weighted* metering: weight 1 - CENTRE_FALLOFF · d², d the distance
// from the centre over the distance to a corner.
const CENTRE_FALLOFF: f32 = 0.75;
// Integer weights for exact, order-free atomics: a frame holds at most (8K / BLOCK)² ·
// WEIGHT_ONE ≈ 1.3e8, inside a u32.
const WEIGHT_ONE: f32 = 64.0;
const MIN_LOG: f32 = -10.0;
const LOG_RANGE: f32 = 20.0;
const KEY: f32 = 0.18;

struct Params {
    // x: min_ev, y: max_ev, z: compensation, w: unused
    limits: vec4<f32>,
    // x: speed_up, y: speed_down (EV/s), z: dt (game seconds), w: snap (1: no easing)
    adapt: vec4<f32>,
    // x: low, y: high (fractions of the samples)
    window: vec4<f32>,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var scene: texture_2d<f32>;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>, BINS>;
// x: the adapted EV, y: the target it eases to, z: 1 once it holds a value
@group(0) @binding(3) var<storage, read_write> state: vec4<f32>;

var<workgroup> local_bins: array<atomic<u32>, BINS>;

fn bin_of(l: f32) -> u32 {
    // Written negated so a NaN (a broken pixel in the block) lands in bin 0 too.
    if (!(l >= exp2(MIN_LOG))) {
        return 0u;
    }
    let t = (log2(l) - MIN_LOG) / LOG_RANGE;
    return u32(clamp(t * f32(BINS), 0.0, f32(BINS - 1u)));
}

fn centre_weight(cell: vec2<u32>, blocks: vec2<u32>) -> u32 {
    let uv = (vec2<f32>(cell) + 0.5) / vec2<f32>(blocks) * 2.0 - 1.0;
    let d2 = dot(uv, uv) * 0.5;
    return u32(round(WEIGHT_ONE * (1.0 - CENTRE_FALLOFF * d2)));
}

@compute @workgroup_size(8, 8, 1)
fn cs_histogram(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    atomicStore(&local_bins[li], 0u);
    workgroupBarrier();
    let dims = textureDimensions(scene);
    let blocks = (dims + BLOCK - 1u) / BLOCK;
    if (all(id.xy < blocks)) {
        let origin = id.xy * BLOCK;
        let end = min(origin + BLOCK, dims);
        var sum = 0.0;
        for (var y = origin.y; y < end.y; y++) {
            for (var x = origin.x; x < end.x; x++) {
                sum += luminance(max(textureLoad(scene, vec2<u32>(x, y), 0).rgb, vec3<f32>(0.0)));
            }
        }
        let n = end - origin;
        atomicAdd(&local_bins[bin_of(sum / f32(n.x * n.y))], centre_weight(id.xy, blocks));
    }
    workgroupBarrier();
    let n = atomicLoad(&local_bins[li]);
    if (n > 0u) {
        atomicAdd(&histogram[li], n);
    }
}

var<workgroup> counts: array<u32, BINS>;

@compute @workgroup_size(64, 1, 1)
fn cs_reduce(@builtin(local_invocation_index) li: u32) {
    counts[li] = atomicExchange(&histogram[li], 0u);
    workgroupBarrier();
    if (li != 0u) {
        return;
    }
    var total = 0.0;
    for (var i = 0u; i < BINS; i++) {
        total += f32(counts[i]);
    }
    // An empty window (low == high) meters the bin the percentile falls in.
    let lo = params.window.x * total;
    let hi = params.window.y * total;
    var below = 0.0;
    var sum = 0.0;
    var weight = 0.0;
    var at_lo = MIN_LOG;
    var found = false;
    for (var i = 0u; i < BINS; i++) {
        let n = f32(counts[i]);
        let centre = MIN_LOG + (f32(i) + 0.5) / f32(BINS) * LOG_RANGE;
        let inside = max(min(below + n, hi) - max(below, lo), 0.0);
        sum += inside * centre;
        weight += inside;
        if (!found && n > 0.0 && below + n >= lo) {
            at_lo = centre;
            found = true;
        }
        below += n;
    }
    let avg = select(at_lo, sum / weight, weight > 0.0);
    let target_ev = clamp(log2(KEY) - avg + params.limits.z, params.limits.x, params.limits.y);

    var ev = target_ev;
    if (params.adapt.w < 0.5 && state.z > 0.5) {
        let dt = params.adapt.z;
        let delta = target_ev - state.x;
        ev = state.x + clamp(delta, -params.adapt.y * dt, params.adapt.x * dt);
    }
    state = vec4<f32>(ev, target_ev, 1.0, 0.0);
}
