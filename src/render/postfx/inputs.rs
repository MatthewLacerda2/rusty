//! The per-frame inputs of the post-FX chain: which passes run, and the views it
//! reads and writes.

use std::collections::BTreeMap;

use crate::render::timing::GpuTimer;

/// Which optional passes run this frame. Bundled rather than passed as loose bools so
/// `run` stays under the 6-arg threshold — and so two same-typed flags can't be
/// swapped silently at the call site.
#[derive(Clone, Debug)]
pub struct PostPasses {
    /// Run the bright-pass + blur that feed the composite's bloom add.
    pub bloom: bool,
    /// Run the final anti-aliasing pass (#360).
    pub fxaa: bool,
    /// Authored post-FX modules to run after tonemapping, in order (#397).
    pub custom: Vec<String>,
    /// Their runtime params, by name (#671): the active volume's `post_params`.
    pub post_params: BTreeMap<String, Vec<f32>>,
}

/// The per-frame views the chain reads/writes, bundled to keep `run` under the
/// 6-arg clippy threshold.
pub struct PostFxContext<'a> {
    pub depth_view: &'a wgpu::TextureView,
    pub skybox_view: &'a wgpu::TextureView,
    pub output: &'a wgpu::TextureView,
    /// The renderer's GPU pass timer (#835): the chain's passes are `post_fx`.
    pub(crate) timer: &'a GpuTimer,
}
