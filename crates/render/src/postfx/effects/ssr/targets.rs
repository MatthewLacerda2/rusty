//! One view's SSR state ([`SsrTargets`]): what the pass reads and writes besides
//! the view's own image, velocity and surface targets, made at a size and remade
//! at a new one (`super::pass`'s `run_ssr`).

use super::pass::{SsrParams, SsrPasses};
use super::pyramid::{colour_copy, HitPyramids};
use crate::gpu::bind_entry as entry;
use crate::gpu::kept::Kept;
use crate::gpu::ops::CountedDevice as _;

/// The views the shared group binds from outside the targets: the velocity target
/// and the two SSR surface targets (weight, applied).
pub(super) type Inputs = [wgpu::TextureView; 3];

/// The copy of the opaque image the trace and the resolve read, the trace's two
/// pyramids the resolve reads, the params uniform, and the two bind groups over
/// them, kept while the view's size holds.
pub(crate) struct SsrTargets {
    pub(super) size: (u32, u32),
    pub(super) colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    pub(super) traced: HitPyramids,
    pub(super) params: wgpu::Buffer,
    /// The trace's and the resolve's shared group, kept while the [`Inputs`] it
    /// binds are (wgpu compares views by identity, so a remade target rebuilds it).
    group: Kept<Inputs, wgpu::BindGroup>,
    /// The resolve's group over the hit pyramids, all these targets' own.
    pub(super) resolve: wgpu::BindGroup,
    /// Test-only: overwrite the image's upper half with `inf` before tracing.
    #[cfg(test)]
    pub(super) poisoned: bool,
}

impl SsrTargets {
    pub(super) fn new(device: &wgpu::Device, passes: &SsrPasses, size: (u32, u32)) -> Self {
        let colour = colour_copy(device, size);
        let traced = HitPyramids::new(device, &passes.pyramid, size);
        let [hits, spread] = traced.views().map(wgpu::BindingResource::TextureView);
        let resolve = device.counted_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("SSR Resolve Bind Group"),
            layout: &passes.traced,
            entries: &[entry(0, hits), entry(1, spread)],
        });
        Self {
            size,
            colour_view: colour.create_view(&Default::default()),
            colour,
            traced,
            group: Kept::default(),
            resolve,
            params: device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("SSR Params"),
                size: std::mem::size_of::<SsrParams>() as u64,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            #[cfg(test)]
            poisoned: false,
        }
    }

    /// The trace's and the resolve's shared group over `inputs`, made only when one
    /// of them was replaced since the last frame.
    pub(super) fn group(
        &mut self,
        device: &wgpu::Device,
        passes: &SsrPasses,
        inputs: Inputs,
    ) -> &wgpu::BindGroup {
        let (params, colour) = (&self.params, &self.colour_view);
        self.group.get(inputs.clone(), || {
            let view = |v| wgpu::BindingResource::TextureView(v);
            let [velocity, weight, applied] = &inputs;
            device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("SSR Bind Group"),
                layout: &passes.layout,
                entries: &[
                    entry(0, params.as_entire_binding()),
                    entry(1, view(colour)),
                    entry(2, wgpu::BindingResource::Sampler(&passes.pyramid.trilinear)),
                    entry(3, view(velocity)),
                    entry(4, view(weight)),
                    entry(5, view(applied)),
                ],
            })
        })
    }
}
