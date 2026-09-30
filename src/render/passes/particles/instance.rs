//! src/render/passes/particles/instance.rs — one emitter's particles as GPU
//! instances (#440). Pure CPU (no GPU state), so the render-mode, flipbook, sorting
//! and lighting rules are unit-tested without an adapter.

use glam::Vec3;

use crate::components::{ParticleBlend, ParticleEmitterComponent, ParticleRenderMode};
use crate::render::draw::sort::{back_to_front, view_depth};
use crate::render::gpu::uniforms::FogUniform;
use crate::scene::Scene;

/// Per-particle instance data uploaded to the GPU (matches `InstanceInput`).
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ParticleInstance {
    pub(crate) center: [f32; 3],
    pub(crate) size: f32,
    /// Sprite rotation in the quad's plane, radians.
    pub(crate) rotation: f32,
    pub(crate) color: [f32; 4],
    /// Stretched: unit velocity direction (xyz) and quad length (w).
    pub(crate) stretch: [f32; 4],
    /// Lit: rgb = probe ambient; w = 0 unlit, 1 flat ambient, 2 probe ambient.
    pub(crate) light: [f32; 4],
    /// Flipbook columns, rows, current frame; soft-fade distance.
    pub(crate) sheet: [f32; 4],
    /// The sprite render mode (0 billboard, 1 stretched, 2 horizontal, 3 vertical).
    pub(crate) mode: u32,
}

impl ParticleInstance {
    const ATTRIBS: [wgpu::VertexAttribute; 8] = wgpu::vertex_attr_array![
        0 => Float32x3, // center
        1 => Float32,   // size
        2 => Float32,   // rotation
        3 => Float32x4, // color
        4 => Float32x4, // stretch
        5 => Float32x4, // light
        6 => Float32x4, // sheet
        7 => Uint32,    // mode
    ];

    pub(crate) fn desc() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleInstance>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Globals uniform: view-projection (and its inverse, to rebuild the scene surface
/// from depth for soft particles), the camera basis for billboarding, and the camera
/// position + scene fog (#437) the vertex stage fogs each corner with.
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

/// Where the camera is and which way it looks — what sorting needs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Eye {
    pub pos: Vec3,
    pub fwd: Vec3,
}

/// The shader's code for a sprite mode (`Mesh` never reaches the sprite pass).
fn mode_code(mode: ParticleRenderMode) -> u32 {
    match mode {
        ParticleRenderMode::Billboard | ParticleRenderMode::Mesh => 0,
        ParticleRenderMode::Stretched => 1,
        ParticleRenderMode::Horizontal => 2,
        ParticleRenderMode::Vertical => 3,
    }
}

/// The `light` lane for an emitter: unlit, or lit by the probe field's average
/// irradiance at `at` (the SH DC band — a puff has no normal) when a probe covers
/// it, else by the flat ambient the shader reads from the lighting uniform.
pub(crate) fn emitter_light(
    scene: &Scene,
    emitter: &ParticleEmitterComponent,
    at: Vec3,
) -> [f32; 4] {
    if !emitter.render.lit {
        return [0.0; 4];
    }
    match scene.probes.sample(at) {
        // Y00 × its cosine-lobe factor π: the irradiance every direction shares.
        Some(sh) => {
            let dc = Vec3::from(sh.coeffs[0]) * 0.282_094_8 * std::f32::consts::PI;
            [dc.x, dc.y, dc.z, 2.0]
        }
        None => [0.0, 0.0, 0.0, 1.0],
    }
}

/// One emitter's live particles as instances plus the emitter's own sort depth (its
/// particles' centroid). Alpha-blended particles are ordered back to front; additive
/// ones sum the same in any order, so they skip the sort.
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
