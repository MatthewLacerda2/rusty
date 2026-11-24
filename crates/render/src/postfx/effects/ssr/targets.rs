use super::pass::{SsrParams, SsrPasses};
use super::pyramid::{colour_copy, HitPyramids};
use crate::gpu::bind_entry as entry;
use crate::gpu::kept::Kept;
use crate::gpu::ops::CountedDevice as _;

pub(super) type Inputs = [wgpu::TextureView; 3];

pub(crate) struct SsrTargets {
    pub(super) size: (u32, u32),
    pub(super) colour: wgpu::Texture,
    colour_view: wgpu::TextureView,
    pub(super) traced: HitPyramids,
    pub(super) params: wgpu::Buffer,
    /// Kept while its inputs are: wgpu compares views by identity, so a remade target rebuilds it.
    group: Kept<Inputs, wgpu::BindGroup>,
    pub(super) resolve: wgpu::BindGroup,
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

    /// The group this frame's SSR bound, for a later pass of the same frame.
    pub(crate) fn frame_group(&self) -> Option<&wgpu::BindGroup> {
        self.group.value()
    }

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
