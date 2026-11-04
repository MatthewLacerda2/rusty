//! The renderer's one integrated volume (#1703): what group 0 binds at 17 and 18
//! and every fogging pass reads. Each base camera running volumetric fog writes it
//! right before its own passes (the camera's work is submitted in order, as its
//! camera uniform is), so one texture serves every view and render texture.
//! While none runs it, it is one texel no pass reads; a grid of another size replaces it
//! and the caller rebuilds group 0.

/// Renders without a camera running volumetric fog before the grid is let go.
pub(crate) const IDLE_RENDERS: u32 = 4;

/// The format of every froxel grid: rgb light, a extinction or transmittance.
pub(crate) const FROXEL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

pub(crate) struct SharedVolume {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    pub(crate) sampler: wgpu::Sampler,
    /// The grid it holds; `None` for the stand-in.
    grid: Option<[u32; 3]>,
    /// Whether a camera wrote it since the last [`Self::tick`].
    used: bool,
    idle: u32,
}

impl SharedVolume {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = stand_in(device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Volumetric Fog Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            view: texture.create_view(&Default::default()),
            texture,
            sampler,
            grid: None,
            used: false,
            idle: 0,
        }
    }

    /// The volume as group 0 binds it.
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// The volume sized for `grid`, made when it holds another; returns whether it
    /// was replaced (group 0 names the old one).
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

    /// One render passed: after [`IDLE_RENDERS`] in which no camera wrote it, go
    /// back to the stand-in. Returns whether it was replaced.
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

    /// The grid it holds, `None` for the stand-in.
    #[cfg(test)]
    pub(crate) fn grid(&self) -> Option<[u32; 3]> {
        self.grid
    }
}

/// A froxel grid of `grid` texels a compute pass writes and a pass samples.
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
        usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}

/// The stand-in while no camera runs volumetric fog: one texel, never read (every
/// reader skips the fetch on the camera's `volumetric.x`, which is 0 then).
fn stand_in(device: &wgpu::Device) -> wgpu::Texture {
    froxel_grid(device, [1, 1, 1], "Volumetric Fog Stand-in")
}
