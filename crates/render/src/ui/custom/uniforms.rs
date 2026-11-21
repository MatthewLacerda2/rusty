use crate::gpu::ops::CountedDevice as _;
use std::collections::HashMap;

use crate::gpu::grow_buffer::GrowBuffer;
use crate::{RenderView, Renderer};
use rusty_shadergen::params::PackedParams;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct UiShadeUniform {
    origin_x: [f32; 4],
    axis_y_size: [f32; 4],
    time: [f32; 4],
    v: PackedParams,
}

pub(crate) struct ShadeUniforms {
    buffer: GrowBuffer,
    pub(super) group: wgpu::BindGroup,
    pub(super) offsets: Offsets,
}

type Offsets = HashMap<(usize, usize), u32>;

pub(super) const SLOT: u64 = std::mem::size_of::<UiShadeUniform>() as u64;

impl Renderer {
    pub(in crate::ui) fn prepare_ui_shades(&mut self, view: &mut RenderView, time: f32) {
        let (bytes, offsets) = self.pack_ui_shades(view, time);
        if offsets.is_empty() {
            if let Some(u) = &mut view.ui.shades {
                u.offsets.clear();
            }
            return;
        }
        self.upload_ui_shades(view, &bytes, offsets);
    }

    fn pack_ui_shades(&mut self, view: &RenderView, time: f32) -> (Vec<u8>, Offsets) {
        let shaders = &mut self.ui_renderer.shaders;
        shaders.refresh();
        let align = self.device.limits().min_uniform_buffer_offset_alignment as usize;
        let stride = (SLOT as usize).next_multiple_of(align);
        let (mut bytes, mut offsets) = (Vec::new(), HashMap::new());
        for (c, canvas) in view.ui.canvases().iter().enumerate() {
            for (b, batch) in canvas.mesh.batches.iter().enumerate() {
                let Some(shade) = &batch.shade else { continue };
                let Some(variant) = shaders.variant(&self.device, &shade.shader.name) else {
                    continue;
                };
                let [o, x, y] = shade.rect;
                let u = UiShadeUniform {
                    origin_x: [o[0], o[1], x[0], x[1]],
                    axis_y_size: [y[0], y[1], shade.size.x, shade.size.y],
                    time: [time, 0.0, 0.0, 0.0],
                    v: variant.params.pack(&shade.shader.params),
                };
                offsets.insert((c, b), bytes.len() as u32);
                bytes.extend_from_slice(bytemuck::bytes_of(&u));
                bytes.resize(bytes.len() + stride - SLOT as usize, 0);
            }
        }
        (bytes, offsets)
    }

    fn upload_ui_shades(&mut self, view: &mut RenderView, bytes: &[u8], offsets: Offsets) {
        let layout = &self.ui_renderer.shaders.shade_layout;
        let bind = |buffer: &wgpu::Buffer| {
            self.device.counted_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("UI Shade Uniforms"),
                layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer,
                        offset: 0,
                        size: wgpu::BufferSize::new(SLOT),
                    }),
                }],
            })
        };
        match &mut view.ui.shades {
            Some(u) => {
                if u.buffer.upload(&self.device, &self.queue, bytes) {
                    u.group = bind(u.buffer.buffer());
                }
                u.offsets = offsets;
            }
            None => {
                let usage = wgpu::BufferUsages::UNIFORM;
                let mut buffer = GrowBuffer::new(&self.device, "UI Shade Uniforms", usage);
                buffer.upload(&self.device, &self.queue, bytes);
                let group = bind(buffer.buffer());
                view.ui.shades = Some(ShadeUniforms {
                    buffer,
                    group,
                    offsets,
                });
            }
        }
    }
}
