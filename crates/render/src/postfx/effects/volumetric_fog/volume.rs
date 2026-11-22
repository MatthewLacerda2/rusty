//! One texel stands in while no camera runs the fog; a grid of another size
//! replaces it and the caller rebuilds group 0.

/// Renders without a camera running volumetric fog before the grid is let go.
pub(crate) const IDLE_RENDERS: u32 = 4;

/// rgb: light; a: extinction in the scatter grids, transmittance in the volume.
pub(crate) const FROXEL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Shared by every view: each camera refills it in its own commands before its passes,
/// so none reads another's.
pub(crate) struct SharedVolume {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    grid: Option<[u32; 3]>,
    used: bool,
    idle: u32,
}

impl SharedVolume {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = stand_in(device);
        Self {
            view: texture.create_view(&Default::default()),
            texture,
            grid: None,
            used: false,
            idle: 0,
        }
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Returns whether it was replaced (group 0 names the old one).
    pub(crate) fn fit(&mut self, device: &wgpu::Device, grid: [u32; 3]) -> bool {
        self.used = true;
        if self.grid == Some(grid) {
            return false;
        }
        self.texture = froxel_grid(device, grid, "Volumetric Fog Volume");
        self.view = self.texture.create_view(&Default::default());
        self.grid = Some(grid);
        true
    }

    /// Returns whether it went back to the stand-in.
    pub(crate) fn tick(&mut self, device: &wgpu::Device) -> bool {
        self.idle = if std::mem::take(&mut self.used) {
            0
        } else {
            self.idle + 1
        };
        if self.grid.is_none() || self.idle < IDLE_RENDERS {
            return false;
        }
        self.texture = stand_in(device);
        self.view = self.texture.create_view(&Default::default());
        self.grid = None;
        true
    }

    #[cfg(test)]
    pub(crate) fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    #[cfg(test)]
    pub(crate) fn grid(&self) -> Option<[u32; 3]> {
        self.grid
    }
}

pub(crate) fn froxel_grid(
    device: &wgpu::Device,
    [w, h, d]: [u32; 3],
    label: &str,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: d,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: FROXEL_FORMAT,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

fn stand_in(device: &wgpu::Device) -> wgpu::Texture {
    froxel_grid(device, [1, 1, 1], "Volumetric Fog Stand-in")
}
