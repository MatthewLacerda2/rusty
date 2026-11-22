use crate::gpu::ops::CountedQueue as _;
use std::collections::{HashMap, HashSet};

use image::imageops::FilterType;

use super::record::NO_LAYER;
use crate::gpu::ops::Writes;
use crate::gpu::textures::kind_watch::KindWatch;
use crate::gpu::textures::{TextureKind, Uploader};
use rusty_asset::MapRef;
use rusty_core::fs::freshness::Freshness;

pub(crate) const LAYER_SIZE: u32 = 512;
const MIPS: u32 = LAYER_SIZE.ilog2() + 1;
pub(crate) const MAX_LAYERS: u32 = 32;
const MIN_LAYERS: u32 = 4;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(crate) struct DecalAtlas {
    texture: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub data_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    slots: HashMap<MapRef, u32>,
    kinds: KindWatch,
}

impl DecalAtlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = create(device, MIN_LAYERS);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Decal Atlas Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });
        let (color_view, data_view) = views(&texture);
        Self {
            texture,
            color_view,
            data_view,
            sampler,
            slots: HashMap::new(),
            kinds: KindWatch::default(),
        }
    }

    pub(crate) fn layer(&self, path: &str) -> u32 {
        self.slots.get(path).copied().unwrap_or(NO_LAYER)
    }

    pub(crate) fn prepare(
        &mut self,
        (device, writes, mips): Uploader,
        maps: &[(&MapRef, TextureKind)],
        misses: &mut Freshness,
    ) -> bool {
        let mut fresh: Vec<(&MapRef, TextureKind)> = Vec::new();
        for &(path, kind) in maps {
            self.kinds.requested(path.as_str(), kind, "decal");
            let known = self.slots.contains_key(path) || misses.is_miss(path.as_str());
            if !known && !fresh.iter().any(|&(p, _)| p == path) {
                fresh.push((path, kind));
            }
        }
        if fresh.is_empty() {
            return false;
        }
        let wanted = self.slots.len() + fresh.len();
        let mut replaced = false;
        if wanted > self.capacity() {
            if wanted > MAX_LAYERS as usize {
                self.slots
                    .retain(|path, _| maps.iter().any(|&(p, _)| p == path));
            }
            let needed = (self.slots.len() + fresh.len()) as u32;
            let capacity = needed.next_power_of_two().clamp(MIN_LAYERS, MAX_LAYERS);
            if capacity > self.capacity() as u32 {
                self.grow(device, writes.0, capacity);
                replaced = true;
            }
        }
        for (path, kind) in fresh {
            let Some(layer) = self.free_layer() else {
                break;
            };
            match crate::gpu::textures::open(path) {
                Some(img) => {
                    upload(writes, &self.texture, layer, img.to_rgba8());
                    mips.generate_layer(device, writes.0, (&self.texture, layer), kind);
                    self.kinds.uploaded(path.as_str(), kind);
                    self.slots.insert(path.clone(), layer);
                }
                None => {
                    misses.miss(path.as_str());
                }
            }
        }
        replaced
    }

    pub(crate) fn forget(&mut self, is_stale: impl Fn(&str) -> bool) {
        self.slots.retain(|path, _| !is_stale(path.as_str()));
    }

    fn capacity(&self) -> usize {
        self.texture.depth_or_array_layers() as usize
    }

    fn free_layer(&self) -> Option<u32> {
        let used: HashSet<u32> = self.slots.values().copied().collect();
        (0..self.capacity() as u32).find(|layer| !used.contains(layer))
    }

    fn grow(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, layers: u32) {
        let bigger = create(device, layers);
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Decal Atlas Grow"),
        });
        let old = self.capacity() as u32;
        for mip in 0..MIPS {
            let side = (LAYER_SIZE >> mip).max(1);
            let at = |texture| wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            };
            let extent = wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: old,
            };
            encoder.copy_texture_to_texture(at(&self.texture), at(&bigger), extent);
        }
        queue.counted_submit(std::iter::once(encoder.finish()));
        (self.color_view, self.data_view) = views(&bigger);
        self.texture = bigger;
    }
}

fn create(device: &wgpu::Device, layers: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Decal Atlas"),
        size: wgpu::Extent3d {
            width: LAYER_SIZE,
            height: LAYER_SIZE,
            depth_or_array_layers: layers,
        },
        mip_level_count: MIPS,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: COLOR_FORMAT,
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[DATA_FORMAT],
    })
}

fn views(texture: &wgpu::Texture) -> (wgpu::TextureView, wgpu::TextureView) {
    let view = |format| {
        texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(format),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        })
    };
    (view(COLOR_FORMAT), view(DATA_FORMAT))
}

fn upload((queue, staging): Writes, texture: &wgpu::Texture, layer: u32, img: image::RgbaImage) {
    let base = image::imageops::resize(&img, LAYER_SIZE, LAYER_SIZE, FilterType::Triangle);
    queue.counted_write_texture(
        staging,
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: 0,
                y: 0,
                z: layer,
            },
            aspect: wgpu::TextureAspect::All,
        },
        &base,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * LAYER_SIZE),
            rows_per_image: Some(LAYER_SIZE),
        },
        wgpu::Extent3d {
            width: LAYER_SIZE,
            height: LAYER_SIZE,
            depth_or_array_layers: 1,
        },
    );
}

#[cfg(test)]
#[path = "atlas_gpu_tests.rs"]
mod gpu_tests;
