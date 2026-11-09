//! src/render/passes/hiz/ — GPU occlusion culling (#1963): two-phase Hi-Z per
//! instance, Unity 6's *GPU Occlusion Culling* beside the baked PVS (#1714).
//!
//! It needs no bake and culls wherever the camera stands. Per camera, after the CPU
//! culls (mask, LOD, frustum, small objects, then the PVS row when the bake is
//! usable: what that drops is never tested here), its `occlusion` stage
//! ([`Renderer::cull_by_hiz`]) runs:
//! 1. **phase one**: each opaque batch keeps, in order, the instances whose object
//!    was drawn last frame (`hiz.wgsl`'s `cs_select`), and the SSAO prepass's
//!    pipelines draw them into the view's depth through indirect args;
//! 2. the depth is max-reduced into a pyramid ([`pyramid`]);
//! 3. every opaque instance's world bounds are tested against it (`cs_test`): the
//!    level where its screen rect spans at most 2x2 texels, hidden only when its
//!    nearest point is behind the farthest depth there, and never when a corner
//!    crosses the near plane. The verdict is next frame's phase-one set;
//! 4. each batch keeps its visible instances, in order (`cs_compact`): the opaque
//!    passes (the SSAO and SSIL prepass, the forward and velocity pass) draw those
//!    through indirect args ([`Culled`]), so an object that came out from behind a
//!    wall this frame is drawn this frame.
//!
//! The phase-one draw is depth only, into the view's depth that the prepass or the
//! forward pass then clears: every pass after it draws the final set, so none of
//! them was split in two. Order within a batch is kept, so a frame where nothing is
//! hidden draws exactly what the CPU batched.
//!
//! **What is tested.** Opaque and cutout batches only: a translucent is never an
//! occluder and always drawn. Phase one draws through the prepass's own pipelines,
//! so only what the prepass writes (lit solids, cutouts clipped, a surface variant's
//! own cut or moved vertices) is ever an occluder. A skinned object is tested with
//! its posed bounds, an object whose bounds are unknown, one the selection outline
//! rings, and a mesh particle are always drawn.
//!
//! **Who culls.** [`Renderer::hiz_culls`]: the quality preset's
//! `gpu_occlusion_culling` (on in every seeded preset), the camera's
//! `use_occlusion_culling`, and never during a capture, which bakes every static
//! mesh. An orthographic camera and one with a far near plane do cull: the test
//! reads the depth the camera drew, so a wall its near plane clips away writes none,
//! and the projected rect is exact for an orthographic projection. An adapter
//! without `INDIRECT_FIRST_INSTANCE` culls nothing this way.
//!
//! **Seams.** The pyramid ([`pyramid::Pyramid`]) is built once per camera and
//! read by mip level and texel, so a later per-cluster test (#1968) binds the same
//! texture; a light's shadow views and the CPU frustum loop could feed `cs_test`
//! the same way, from their own depth.

mod counters;
mod cull;
pub(crate) mod pyramid;
mod upload;

#[cfg(all(test, feature = "dev"))]
mod dust2_gpu_tests;
#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod moving_gpu_tests;
#[cfg(test)]
mod projection_gpu_tests;
#[cfg(test)]
mod scale_gpu_tests;

use crate::render::gpu::pipelines::LazyCompute;
use crate::render::gpu::shaders::ShaderRegistry;
use crate::render::Renderer;

pub(crate) use cull::{CameraCull, Culled, ARGS};

/// The device feature the indirect draws need: each batch's kept instances start
/// at its own run of the array, so its args name a first instance.
pub(crate) const FEATURE: wgpu::Features = wgpu::Features::INDIRECT_FIRST_INSTANCE;

/// The compute pipelines, shared by every view, built on their first dispatch.
pub(crate) struct HiZ {
    pub(super) cull_layout: wgpu::BindGroupLayout,
    pub(super) select: LazyCompute,
    pub(super) test: LazyCompute,
    pub(super) compact: LazyCompute,
    pub(super) reduce_depth: LazyCompute,
    pub(super) reduce: LazyCompute,
}

/// What one view keeps for it: the pyramid, and each stacked camera's buffers,
/// by its place in the stack.
#[derive(Default)]
pub(crate) struct HiZView {
    pub(crate) pyramid: Option<pyramid::Pyramid>,
    pub(crate) cameras: Vec<CameraCull>,
}

impl HiZ {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let mut registry = ShaderRegistry::new(crate::shadergen::engine_shader_dir());
        let cull = registry.load(device, "hiz.wgsl", "Hi-Z Cull");
        let levels = registry.load(device, "hiz_pyramid.wgsl", "Hi-Z Pyramid");
        let cull_layout = upload::layout(device);
        let with = |entry| lazy(device, &cull, entry, Some(&cull_layout));
        let (select, test, compact) = (with("cs_select"), with("cs_test"), with("cs_compact"));
        Self {
            select,
            test,
            compact,
            reduce_depth: lazy(device, &levels, "cs_reduce_depth", None),
            reduce: lazy(device, &levels, "cs_reduce", None),
            cull_layout,
        }
    }

    /// The pipelines, for warming (`Renderer::lazy_computes`).
    pub(crate) fn lazy_computes(&self) -> [&LazyCompute; 5] {
        let Self {
            select,
            test,
            compact,
            reduce_depth,
            reduce,
            ..
        } = self;
        [select, test, compact, reduce_depth, reduce]
    }
}

impl Renderer {
    /// Whether `cam` is culled by the GPU's depth pyramid ([module docs](self)).
    pub(crate) fn hiz_culls(&self, cam: &crate::scene::Camera) -> bool {
        let supported = self.device.features().contains(FEATURE);
        let on = self.quality.gpu_occlusion_culling && cam.use_occlusion_culling;
        supported && on && !self.capture.static_only
    }
}

/// A compute pipeline of `module`'s `entry`, over `layout` (or the one its entry
/// declares), built on first use.
fn lazy(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    entry: &'static str,
    layout: Option<&wgpu::BindGroupLayout>,
) -> LazyCompute {
    let layout = layout.map(|group| {
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(entry),
            bind_group_layouts: &[Some(group)],
            immediate_size: 0,
        })
    });
    let module = module.clone();
    LazyCompute::new(device, move |device| {
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some(entry),
            layout: layout.as_ref(),
            module: &module,
            entry_point: Some(entry),
            compilation_options: Default::default(),
            cache: None,
        })
    })
}
