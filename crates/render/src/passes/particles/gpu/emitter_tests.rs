//! Every device uses wgpu's default limits, so the cap's buffers must fit them.

use super::*;
use rusty_components::particle::GPU_MAX_PARTICLES;

#[test]
fn the_caps_buffers_fit_the_default_limits() {
    let limits = wgpu::Limits::default();
    let most = limits
        .max_storage_buffer_binding_size
        .min(limits.max_buffer_size);
    let particles = bytes::<GpuParticle>(GPU_MAX_PARTICLES);
    let births = bytes::<GpuBirth>(births_slots(GPU_MAX_PARTICLES));
    assert!(
        particles <= most,
        "{particles} bytes of particles, {most} fit"
    );
    assert!(births <= most, "{births} bytes of births, {most} fit");
}
