//! The Hi-Z pyramid (#1963): a view's depth max-reduced level by level. Level 0 is
//! half the depth's size rounded up to a power of two, and each level after it
//! half the one below (a full mip chain), so level `l`'s texel `t` covers the depth
//! pixels `t << (l + 1)` up to the next texel's; texels past the depth's edge
//! repeat it. Kept per view and made again when the depth's size changes.

use super::kept::Kept;
use super::HiZ;

/// The pyramid's format: one float, written by a compute pass.
const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::R32Float;

pub(crate) struct Pyramid {
    /// The depth size it was made for.
    pub(crate) depth: (u32, u32),
    texture: wgpu::Texture,
    /// All its levels, as `cs_test` reads them.
    pub(crate) view: wgpu::TextureView,
    /// One view per level, each reduced from the one below.
    levels: Vec<wgpu::TextureView>,
    /// Each level's reduction group, kept while the depth it reads is.
    groups: Kept<wgpu::TextureView, Vec<wgpu::BindGroup>>,
}

/// Half of `n`, rounded up, never below one.
fn half(n: u32) -> u32 {
    n.div_ceil(2).max(1)
}

/// Level 0's size over a depth of `w × h`.
fn base(w: u32, h: u32) -> (u32, u32) {
    (half(w).next_power_of_two(), half(h).next_power_of_two())
}

/// The levels of a pyramid over a depth of `w × h`: from [`base`] to 1×1.
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

    /// Record the reduction of `depth` into every level.
    pub(crate) fn build(
        &mut self,
        device: &wgpu::Device,
        hiz: &HiZ,
        encoder: &mut wgpu::CommandEncoder,
        depth: &wgpu::TextureView,
    ) {
        let levels = &self.levels;
        let groups = self.groups.get(depth.clone(), || {
            (0..levels.len())
                .map(|level| level_group(device, hiz, (depth, levels), level))
                .collect()
        });
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("Hi-Z Pyramid"),
            timestamp_writes: None,
        });
        let mut size = base(self.depth.0, self.depth.1);
        for (level, group) in groups.iter().enumerate() {
            pass.set_pipeline(level_pipeline(hiz, level));
            pass.set_bind_group(0, group, &[]);
            pass.dispatch_workgroups(size.0.div_ceil(8), size.1.div_ceil(8), 1);
            size = (half(size.0), half(size.1));
        }
    }
}

/// The reduction that writes `level`: from the depth, or from the level below.
fn level_pipeline(hiz: &HiZ, level: usize) -> &wgpu::ComputePipeline {
    match level {
        0 => hiz.reduce_depth.get(),
        _ => hiz.reduce.get(),
    }
}

/// The group that reduces into `level`: `depth` or the level below in, the
/// level out.
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
    device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        assert_eq!(level_count(3, 1), 2); // 2x1, then 1x1
        assert_eq!(level_count(96, 64), 7); // 64x32 down to 1x1
        assert_eq!(level_count(1920, 1080), 11); // 1024x1024 down to 1x1
    }
}
