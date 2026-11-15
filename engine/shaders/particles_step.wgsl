// engine/shaders/particles_step.wgsl — a GPU-simulated emitter's update (#2170).
//
// `cs_birth` writes the births the CPU made since the last upload into their slots;
// `cs_step` then moves every live particle up to the step the sim has reached. The
// steps in the uploaded window (`dts`, at most the sim's `DT_HISTORY`) are taken
// one at a time, each by the `dt` that step took, so on one adapter a run draws the
// same particles whatever the frame rate; steps older than the window (a renderer
// that drew nothing for longer, a long `Harness.Step`) are one closed-form `jump`
// at the window's oldest `dt`, so the loop is bounded by the window. A step is the
// CPU path's (`app::systems::particles::integrate_and_collide`) minus collision:
// gravity, the drag curve, the spin curve, velocity, age; a particle whose age
// reaches its lifetime is dead. The Rust mirror is
// `render::passes::particles::gpu::twin::advance`.

#import particles::gpu::{GpuParticle, GpuBirth, EMPTY, gpu_emitter, life_t, lut_at}

@group(0) @binding(2) var<storage, read_write> particles: array<GpuParticle>;
@group(0) @binding(4) var<storage, read> dts: array<f32>;
@group(0) @binding(5) var<storage, read> births: array<GpuBirth>;

@compute @workgroup_size(64)
fn cs_birth(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gpu_emitter.counts.x) {
        return;
    }
    let b = births[id.x];
    particles[b.slot] = b.particle;
}

// `age < lifetime` failing, a NaN lifetime included, as on the CPU.
fn is_dead(p: GpuParticle) -> bool {
    return !(p.age < p.lifetime);
}

// `n` steps of `dt` at once, the curves held where `p` is in its life: the sum of
// the step's geometric velocity series (v <- (v + g dt) k, k the drag factor) and
// of the positions it passes.
fn jump(p: ptr<function, GpuParticle>, n: u32, g: vec3<f32>, dt: f32) {
    let curves = lut_at(0u, life_t(*p));
    let nf = f32(n);
    let k = max(1.0 - curves.y * dt, 0.0);
    let v = (*p).velocity;
    if (abs(1.0 - k) < 1e-6) {
        (*p).velocity = v + g * dt * nf;
        (*p).position = (*p).position + (v * nf + g * dt * nf * (nf + 1.0) * 0.5) * dt;
    } else {
        let rest = g * dt * k / (1.0 - k);
        let kn = select(0.0, pow(k, nf), k > 0.0);
        (*p).velocity = (v - rest) * kn + rest;
        (*p).position = (*p).position + ((v - rest) * k * (1.0 - kn) / (1.0 - k) + rest * nf) * dt;
    }
    (*p).rotation = (*p).rotation + radians(curves.z) * dt * nf;
    (*p).age = (*p).age + dt * nf;
}

@compute @workgroup_size(64)
fn cs_step(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gpu_emitter.info.y) {
        return;
    }
    var p = particles[id.x];
    let goal = gpu_emitter.info.z;
    let first = gpu_emitter.info.w;
    let kept = gpu_emitter.counts.y;
    if (p.next == EMPTY || p.next > goal || kept == 0u) {
        return;
    }
    let g = gpu_emitter.gravity.xyz;
    if (p.next < first) {
        jump(&p, first - p.next, g, dts[0]);
        p.next = select(first, EMPTY, is_dead(p));
    }
    // At most `kept` steps: `first + kept - 1` is the goal.
    for (var k = 0u; k < kept && p.next != EMPTY && p.next <= goal; k = k + 1u) {
        let dt = dts[min(p.next - first, kept - 1u)];
        let curves = lut_at(0u, life_t(p));
        p.velocity = p.velocity + g * dt;
        p.velocity = p.velocity * max(1.0 - curves.y * dt, 0.0);
        p.rotation = p.rotation + radians(curves.z) * dt;
        p.position = p.position + p.velocity * dt;
        p.age = p.age + dt;
        p.next = select(p.next + 1u, EMPTY, is_dead(p));
    }
    particles[id.x] = p;
}
