// engine/shaders/particles_step.wgsl — a GPU-simulated emitter's update (#2170).
//
// `cs_birth` writes the births the CPU made since the last upload into their slots;
// `cs_step` then moves every live particle one sim step at a time up to the step the
// sim has reached, each step by the `dt` that step took (`dts`), so on one adapter a
// run draws the same particles whatever the frame rate. A step is the CPU path's
// (`app::systems::particles::integrate_and_collide`) minus collision: gravity, the
// drag curve, the spin curve, velocity, age; a particle whose age reaches its
// lifetime is dead. The Rust mirror is `render::passes::particles::gpu::record::advance`.

#import particles::gpu::{GpuParticle, GpuBirth, DEAD, gpu_emitter, life_t, lut_at}

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

@compute @workgroup_size(64)
fn cs_step(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gpu_emitter.info.y) {
        return;
    }
    var p = particles[id.x];
    let goal = gpu_emitter.info.z;
    let first = gpu_emitter.info.w;
    let kept = gpu_emitter.counts.y;
    // Wrapping: a slot ahead of the goal (it cannot be) is left alone.
    let owed = goal - p.done;
    if (p.done == DEAD || owed == 0u || owed > 0x80000000u || kept == 0u) {
        return;
    }
    let g = gpu_emitter.gravity.xyz;
    for (var k = 0u; k < owed; k = k + 1u) {
        let step = p.done + 1u;
        // The dt of `step`, or the oldest kept one for a step older than the history.
        let at = select(step - first, 0u, step - first >= 0x80000000u);
        let dt = dts[min(at, kept - 1u)];
        let curves = lut_at(0u, life_t(p));
        p.velocity = p.velocity + g * dt;
        p.velocity = p.velocity * max(1.0 - curves.y * dt, 0.0);
        p.rotation = p.rotation + radians(curves.z) * dt;
        p.position = p.position + p.velocity * dt;
        p.age = p.age + dt;
        p.done = step;
        if (!(p.age < p.lifetime)) {
            p.done = DEAD;
            break;
        }
    }
    particles[id.x] = p;
}
