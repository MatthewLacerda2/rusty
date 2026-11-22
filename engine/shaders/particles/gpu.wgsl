// A GPU emitter's step (`particles_step.wgsl`) and draw (`particles.wgsl`) share these; their
// Rust twins, field for field, are in `passes::particles::gpu::record`.
#define_import_path particles::gpu

// A slot's `next` when nothing lives in it: zero, so an all-zero buffer is empty.
const EMPTY: u32 = 0u;
// Samples per over-life curve.
const LUT_SIZE: u32 = 64u;

struct GpuParticle {
    position: vec3<f32>,
    lifetime: f32,
    velocity: vec3<f32>,
    age: f32,
    color: vec4<f32>,
    size: f32,
    rotation: f32,
    start_frame: u32,
    // The first step it has yet to take (steps count from 1), or EMPTY.
    next: u32,
};

// What a birth upload carries: the particle, its slot, and the time its steps
// older than the uploaded window took (0 when it has none).
struct GpuBirth {
    particle: GpuParticle,
    slot: u32,
    // Padded to the Rust record's 16 bytes (a vec2 pad would align to 8 and
    // make the record 96 bytes).
    @size(12) jumped: f32,
};

struct GpuEmitter {
    // Where the stored positions are drawn: the emitter's `gpu_space` when `Local`, else identity.
    frame: mat4x4<f32>,
    // xyz: gravity in the stored space; w: the time the steps older than the
    // window took for a particle stepped at the last upload.
    gravity: vec4<f32>,
    light: vec4<f32>,
    // Flipbook columns, rows, cycles; soft-fade distance.
    sheet: vec4<f32>,
    // Stretch length scale, speed scale; TAA reactivity; unused.
    stretch: vec4<f32>,
    // Render mode code; slots in use; the step to move to; the step of `dts[0]`.
    info: vec4<u32>,
    // Births to scatter this upload; `dts` in use; unused, unused.
    counts: vec4<u32>,
};

@group(3) @binding(1) var<uniform> gpu_emitter: GpuEmitter;
@group(3) @binding(3) var<storage, read> gpu_lut: array<vec4<f32>>;

// Normalised life in [0, 1], as `Particle::life_t`.
fn life_t(p: GpuParticle) -> f32 {
    if (p.lifetime <= 0.0) {
        return 1.0;
    }
    return clamp(p.age / p.lifetime, 0.0, 1.0);
}

// The lookup table at `t` for curve row `row` (0: size, drag, spin; 1: colour),
// linear between samples.
fn lut_at(row: u32, t: f32) -> vec4<f32> {
    let x = clamp(t, 0.0, 1.0) * f32(LUT_SIZE - 1u);
    let i = min(u32(floor(x)), LUT_SIZE - 2u);
    let base = row * LUT_SIZE;
    return mix(gpu_lut[base + i], gpu_lut[base + i + 1u], x - f32(i));
}
