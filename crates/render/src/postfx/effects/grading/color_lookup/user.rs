use crate::gpu::ops::{CountedQueue as _, Writes};
use std::collections::BTreeMap;

use rusty_asset::lut::{self, Lut};
use rusty_components::PostProcessingComponent;
use rusty_core::fs::freshness::Freshness;
use rusty_core::fs::written::Stale;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lookup<'a> {
    pub path: &'a str,
    pub contribution: f32,
}

impl<'a> Lookup<'a> {
    pub fn of(volume: Option<&'a PostProcessingComponent>) -> Option<Self> {
        let vc = volume.filter(|vc| vc.color_lookup.contribution > 0.0)?;
        let path = vc.color_lookup.lut.as_deref().filter(|p| !p.is_empty())?;
        Some(Self {
            path,
            contribution: vc.color_lookup.contribution,
        })
    }
}

pub struct UserLut {
    pub(crate) id: u64,
    pub(crate) view: wgpu::TextureView,
    pub(crate) domain: [[f32; 3]; 2],
}

#[derive(Default)]
pub struct UserLuts {
    freshness: Freshness,
    loaded: BTreeMap<String, Option<UserLut>>,
    uploads: u64,
}

impl UserLuts {
    pub(crate) fn get(
        &mut self,
        device: &wgpu::Device,
        writes: Writes,
        path: &str,
    ) -> Option<&UserLut> {
        if !self.loaded.contains_key(path) {
            let upload = match lut::load(std::path::Path::new(path)) {
                Ok(lut) => {
                    self.uploads += 1;
                    Some(upload(device, writes, &lut, self.uploads))
                }
                Err(e) => {
                    log::warn!("[Render] Color Lookup `{path}` ignored: {e}");
                    if !std::path::Path::new(path).is_file() {
                        self.freshness.miss(path);
                    }
                    None
                }
            };
            self.loaded.insert(path.to_string(), upload);
        }
        self.loaded.get(path)?.as_ref()
    }

    pub fn refresh(&mut self) {
        let changes = self
            .freshness
            .poll_with(|p| std::path::Path::new(p).exists());
        if changes.stale.is_empty() {
            return;
        }
        let stale = Stale::new(changes.stale);
        self.loaded.retain(|key, _| !stale.matches(key));
        self.freshness.forget_misses_where(|p| stale.matches(p));
    }
}

fn upload(device: &wgpu::Device, (queue, staging): Writes, lut: &Lut, id: u64) -> UserLut {
    let n = lut.size;
    let texture = texture(device, n);
    queue.counted_write_texture(
        staging,
        texture.as_image_copy(),
        bytemuck::cast_slice(&lut.texels),
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(n * 16),
            rows_per_image: Some(n),
        },
        texture.size(),
    );
    UserLut {
        id,
        view: texture.create_view(&Default::default()),
        domain: lut.domain,
    }
}

pub(crate) fn neutral(device: &wgpu::Device) -> wgpu::TextureView {
    texture(device, 1).create_view(&Default::default())
}

fn texture(device: &wgpu::Device, n: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Color Lookup"),
        size: wgpu::Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: n,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D3,
        format: wgpu::TextureFormat::Rgba32Float,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    })
}
