// engine/shaders/postfx/exposure.wgsl — automatic exposure (#1444), Unity HDRP's
// *Automatic Histogram*: two compute passes over one view's HDR scene, run each
// frame its volume is in Automatic mode (`ExposureMeter::run` in
// `src/render/postfx/exposure/mod.rs`, with pipelines from `ExposurePasses`).
//
//   cs_histogram — a GRID × GRID sample of the scene, each sample's luminance binned
//                  by log2 into BINS bins spanning [MIN_LOG, MIN_LOG + LOG_RANGE)
//   cs_reduce    — one workgroup: averages the bins between the low and high
//                  percentiles, turns that into the EV that brings it to middle grey,
//                  eases the adapted EV towards it and clears the histogram
//
// The adapted EV stays on the GPU (`state.x`), where the composite reads it: no
// readback on the frame's path. Integer atomics and a serial reduce keep it a pure
// function of the scene and the params, so a capture replays exactly.

#import postfx::grade::luminance

// Mirrors the `BINS` / `GRID` consts in `src/render/postfx/exposure/mod.rs`.
const BINS: u32 = 64u;
const GRID: u32 = 64u;
// The log2-luminance span the histogram meters: 2^-10 to 2^10.
const MIN_LOG: f32 = -10.0;
const LOG_RANGE: f32 = 20.0;
// Middle grey: the luminance the metered average is exposed to.
const KEY: f32 = 0.18;

// Mirrors `ExposureParams`.
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
    if (l < exp2(MIN_LOG)) {
        return 0u;
    }
    let t = (log2(l) - MIN_LOG) / LOG_RANGE;
    return u32(clamp(t * f32(BINS), 0.0, f32(BINS - 1u)));
}

@compute @workgroup_size(8, 8, 1)
fn cs_histogram(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(local_invocation_index) li: u32,
) {
    atomicStore(&local_bins[li], 0u);
    workgroupBarrier();
    // The sample at the centre of this cell of the grid: a fixed downsample, so the
    // cost is the same at any resolution.
    let dims = textureDimensions(scene);
    let cell = (vec2<f32>(id.xy) + 0.5) / f32(GRID);
    let coord = min(vec2<u32>(cell * vec2<f32>(dims)), dims - 1u);
    let c = max(textureLoad(scene, coord, 0).rgb, vec3<f32>(0.0));
    atomicAdd(&local_bins[bin_of(luminance(c))], 1u);
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
    // The samples between the low and high percentiles, each bin weighted by how
    // much of it falls inside that window. An empty window (low == high) meters the
    // bin the percentile falls in.
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
