//! Light lane w: 0 unlit, 1 flat ambient, 2 probe ambient. Probe irradiance is the SH DC band over
//! π, the matte response the shader gives every light: the coefficient × Y00 × π is E, and a matte
//! puff reflects E/π.
//! Alpha particles sort back to front; additive ones skip the sort.

use glam::Vec3;

use crate::draw::batch::sort::{back_to_front, view_depth};
use crate::gpu::uniforms::FogUniform;
use rusty_components::{ParticleBlend, ParticleEmitterComponent, ParticleRenderMode};
use rusty_scene::Scene;

/// Fields and `ATTRIBS` match `particles.wgsl`'s `InstanceInput`: change the two together.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ParticleInstance {
    pub(crate) center: [f32; 3],
    pub(crate) size: f32,
    /// Radians, in the quad's plane.
    pub(crate) rotation: f32,
    pub(crate) color: [f32; 4],
    pub(crate) stretch: [f32; 4],
    pub(crate) light: [f32; 4],
    pub(crate) sheet: [f32; 4],
    pub(crate) mode: u32,
    /// TAA's reactivity; below 0 is `Auto`.
    pub(crate) reactive: f32,
}

impl ParticleInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 9] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32,
        2 => Float32,
        3 => Float32x4,
        4 => Float32x4,
        5 => Float32x4,
        6 => Float32x4,
        7 => Uint32,
        8 => Float32,
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ParticleGlobals {
    pub(crate) view_proj: [f32; 16],
    pub(crate) inv_view_proj: [f32; 16],
    pub(crate) cam_right: [f32; 4],
    pub(crate) cam_up: [f32; 4],
    pub(crate) cam_pos: [f32; 4],
    pub(crate) cam_fwd: [f32; 4],
    pub(crate) fog: FogUniform,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Eye {
    pub pos: Vec3,
    pub fwd: Vec3,
}

pub(crate) fn mode_code(mode: ParticleRenderMode) -> u32 {
    match mode {
        ParticleRenderMode::Billboard | ParticleRenderMode::Mesh => 0,
        ParticleRenderMode::Stretched => 1,
        ParticleRenderMode::Horizontal => 2,
        ParticleRenderMode::Vertical => 3,
    }
}

pub(crate) fn emitter_light(
    scene: &Scene,
    emitter: &ParticleEmitterComponent,
    at: Vec3,
) -> [f32; 4] {
    if !emitter.render.lit {
        return [0.0; 4];
    }
    match scene.sample_probes(at) {
        Some(sh) => {
            let dc = Vec3::from(sh.coeffs[0]) * 0.282_094_8; // Y00, 1/(2√π).
            [dc.x, dc.y, dc.z, 2.0]
        }
        None => [0.0, 0.0, 0.0, 1.0],
    }
}

pub(crate) fn emitter_instances(
    emitter: &ParticleEmitterComponent,
    light: [f32; 4],
    eye: Eye,
) -> (Vec<ParticleInstance>, f32) {
    let render = &emitter.render;
    let book = &render.flipbook;
    let mode = mode_code(render.mode);
    let mut centroid = Vec3::ZERO;
    let mut keyed: Vec<(ParticleInstance, f32)> = emitter
        .runtime
        .particles
        .iter()
        .map(|p| {
            centroid += p.position;
            let size = emitter.size_of(p);
            let stretch = p.velocity.normalize_or_zero();
            let length = render.stretch_length(size, p.velocity);
            let instance = ParticleInstance {
                center: p.position.to_array(),
                size,
                rotation: p.rotation,
                color: emitter.color_of(p),
                stretch: stretch.extend(length).to_array(),
                light,
                sheet: [
                    book.columns.max(1) as f32,
                    book.rows.max(1) as f32,
                    book.frame_of(p) as f32,
                    render.soft_distance,
                ],
                mode,
                reactive: render.reactive.gpu(),
            };
            (instance, view_depth(p.position, eye.pos, eye.fwd))
        })
        .collect();
    let count = keyed.len().max(1) as f32;
    if emitter.blend == ParticleBlend::Alpha {
        back_to_front(&mut keyed);
    }
    let depth = view_depth(centroid / count, eye.pos, eye.fwd);
    (keyed.into_iter().map(|(i, _)| i).collect(), depth)
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
