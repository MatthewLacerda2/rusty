use super::HiZ;
use crate::gpu::kept::Kept;
use crate::gpu::ops::CountedDevice as _;

/// `hiz_pyramid.wgsl`'s `r32float` storage texture: change the two together.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

pub(crate) struct Pyramid {
    pub(crate) depth: (u32, u32),
    texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    levels: Vec<wgpu::TextureView>,
    groups: Kept<wgpu::TextureView, Vec<wgpu::BindGroup>>,
}

fn half(n: u32) -> u32 {
    n.div_ceil(2).max(1)
}

fn base(w: u32, h: u32) -> (u32, u32) {
    (half(w).next_power_of_two(), half(h).next_power_of_two())
}

pub(crate) fn level_count(w: u32, h: u32) -> u32 {
    let (w, h) = base(w, h);
    w.max(h).ilog2() + 1
}

impl Pyramid {
    pub(crate) fn new(device: &wgpu::Device, (w, h): (u32, u32)) -> Self {
        let count = level_count(w, h);
        let (bw, bh) = base(w, h);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Hi-Z Pyramid"),
            size: wgpu::Extent3d {
                width: bw,
                height: bh,
                depth_or_array_layers: 1,
            },
            mip_level_count: count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FORMAT,
            usage: wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let levels = (0..count)
            .map(|level| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        Self {
            depth: (w, h),
            view: texture.create_view(&Default::default()),
            texture,
            levels,
            groups: Kept::default(),
        }
    }

    pub(crate) fn levels(&self) -> u32 {
        self.texture.mip_level_count()
    }

    pub(crate) fn build(
        &mut self,
        device: &wgpu::Device,
        hiz: &HiZ,
        pass: &mut wgpu::ComputePass,
        depth: &wgpu::TextureView,
    ) {
        let levels = &self.levels;
        let groups = self.groups.get(depth.clone(), || {
            (0..levels.len())
                .map(|level| level_group(device, hiz, (depth, levels), level))
                .collect()
        });
        let mut size = base(self.depth.0, self.depth.1);
        for (level, group) in groups.iter().enumerate() {
            pass.set_pipeline(level_pipeline(hiz, level));
            pass.set_bind_group(crate::gpu::ONLY_GROUP, group, &[]);
            // 8: `hiz_pyramid.wgsl`'s workgroup size.
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
            size = (half(size.0), half(size.1));
        }
    }
}

fn level_pipeline(hiz: &HiZ, level: usize) -> &wgpu::ComputePipeline {
    match level {
        0 => hiz.reduce_depth.get(),
        _ => hiz.reduce.get(),
    }
}

fn level_group(
    device: &wgpu::Device,
    hiz: &HiZ,
    (depth, levels): (&wgpu::TextureView, &[wgpu::TextureView]),
    level: usize,
) -> wgpu::BindGroup {
    let (binding, input) = match level {
        0 => (0, depth),
        _ => (1, &levels[level - 1]),
    };
    device.counted_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Hi-Z Level"),
        layout: &level_pipeline(hiz, level).get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(input),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&levels[level]),
            },
        ],
    })
}

#[cfg(test)]
mod tests {
    use super::level_count;

    #[test]
    fn levels_run_from_half_the_depth_to_one_texel() {
        assert_eq!(level_count(1, 1), 1);
        assert_eq!(level_count(2, 2), 1);
        assert_eq!(level_count(3, 1), 2);
        assert_eq!(level_count(96, 64), 7);
        assert_eq!(level_count(1920, 1080), 11);
    }
}
