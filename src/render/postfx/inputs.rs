//! The per-frame views the post-FX chain reads and writes.

use crate::render::timing::GpuTimer;

/// The per-frame views the chain reads/writes, bundled to keep `run` under the
/// 6-arg clippy threshold.
pub struct PostFxContext<'a> {
    pub depth_view: &'a wgpu::TextureView,
    /// The skybox panorama and its own (wrapping) sampler (`postfx/frame.wgsl`).
    pub skybox: (&'a wgpu::TextureView, &'a wgpu::Sampler),
    pub output: &'a wgpu::TextureView,
    /// The renderer's GPU pass timer (#835): the chain's passes are `post_fx`.
    pub(crate) timer: &'a GpuTimer,
    /// The renderer's shared effect pipelines (`Renderer::post_passes`), which an
    /// LDR pass draws through (TAA's sharpen, #1847).
    pub(crate) passes: &'a super::effects::Passes,
}
