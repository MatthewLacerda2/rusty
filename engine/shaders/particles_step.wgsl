// A GPU emitter's update. Each step in the uploaded window takes its own `dt`, so on one adapter a
// run draws the same particles whatever the frame rate; older steps are one closed-form `jump`. A
// step is `app::systems::particles::integrate_and_collide` minus collision; its CPU twin, which
// changes with it, is `render::passes::particles::gpu::twin`.
#import particles::gpu::{GpuParticle, GpuBirth, EMPTY, gpu_emitter, life_t, lut_at}

@group(3) @binding(2) var<storage, read_write> particles: array<GpuParticle>;
@group(3) @binding(4) var<storage, read> dts: array<f32>;
@group(3) @binding(5) var<storage, read> births: array<GpuBirth>;

@compute @workgroup_size(64)
fn cs_birth(@builtin(global_invocation_id) id: vec3<u32>) {
    if (id.x >= gpu_emitter.counts.x) {
        return;
    }
    let b = births[id.x];
    var p = b.particle;
    let first = gpu_emitter.info.w;
    // A birth older than the window jumps by its own time, so `cs_step`'s jump (the emitter's
    // time) is only ever the particles stepped at the last upload.
    if (p.next != EMPTY && p.next < first) {
        jump(&p, first - p.next, gpu_emitter.gravity.xyz, b.jumped);
        p.next = select(first, EMPTY, is_dead(p));
    }
    particles[b.slot] = p;
}

// `age < lifetime` failing, a NaN lifetime included, as on the CPU.
fn is_dead(p: GpuParticle) -> bool {
    return !(p.age < p.lifetime);
}

// `n` steps of `span` seconds at once, curves held: the sum of the series
// v <- (v + g dt) k (k the drag factor) at the mean `dt`.
fn jump(p: ptr<function, GpuParticle>, n: u32, g: vec3<f32>, span: f32) {
    let curves = lut_at(0u, life_t(*p));
    let nf = f32(n);
    let dt = span / nf;
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
    (*p).rotation = (*p).rotation + radians(curves.z) * span;
    (*p).age = (*p).age + span;
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
        jump(&p, first - p.next, g, gpu_emitter.gravity.w);
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
