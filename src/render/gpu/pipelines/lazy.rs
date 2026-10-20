//! A render pipeline built on its first draw, not with the renderer (#985).
//!
//! Creating a pipeline compiles its shaders for the adapter, and on a software
//! rasterizer (lavapipe) that is an LLVM compile of the whole forward shader: about
//! 90 ms each. A renderer that never draws a translucent solid, a debug line or a
//! selection outline has no use for those pipelines, so they wait for the first pass
//! that binds them. Every headless renderer is one test process under nextest, so
//! this is paid per GPU test, and the suite's wall time is the GPU group's.

use std::sync::OnceLock;

/// Builds the pipeline, the first time it is asked for.
type Build = Box<dyn Fn(&wgpu::Device) -> wgpu::RenderPipeline + Send + Sync>;

/// A pipeline that is created the first time [`LazyPipeline::get`] runs, then kept.
pub(crate) struct LazyPipeline {
    cell: OnceLock<wgpu::RenderPipeline>,
    device: wgpu::Device,
    build: Build,
}

impl LazyPipeline {
    /// A pipeline `build` creates on `device` at first use; nothing is compiled yet.
    pub(crate) fn new(
        device: &wgpu::Device,
        build: impl Fn(&wgpu::Device) -> wgpu::RenderPipeline + Send + Sync + 'static,
    ) -> Self {
        Self {
            cell: OnceLock::new(),
            device: device.clone(),
            build: Box::new(build),
        }
    }

    /// The pipeline, built if this is the first call.
    pub(crate) fn get(&self) -> &wgpu::RenderPipeline {
        self.cell.get_or_init(|| (self.build)(&self.device))
    }

    /// Whether the pipeline has been built yet.
    #[cfg(test)]
    pub(crate) fn is_built(&self) -> bool {
        self.cell.get().is_some()
    }
}
