//! The decal atlas (#638): every decal map in one texture array, so the forward
//! shader samples any decal's maps through one binding and no decal needs a bind
//! group of its own. Each map is resized to a [`LAYER_SIZE`]² layer with a full mip
//! chain, keyed by path, and loaded once.
//!
//! The array is sRGB with a linear view beside it, as material maps are (#647):
//! albedo is read through the sRGB view and decoded, the normal, metallic and
//! roughness maps through the linear one, raw. It starts small and doubles (copying
//! what it holds) up to [`MAX_LAYERS`]; past that, layers no decal of the frame
//! uses are reclaimed, and a map that still finds no room is drawn without (counted
//! in `RenderCounters::decal_maps_dropped`).

use std::collections::{HashMap, HashSet};

use image::imageops::FilterType;

use super::record::NO_LAYER;

/// The side of one layer, in texels.
pub(crate) const LAYER_SIZE: u32 = 512;
/// Mip levels of a layer: 512 down to 1.
const MIPS: u32 = LAYER_SIZE.ilog2() + 1;
/// The most maps the atlas holds at once.
pub(crate) const MAX_LAYERS: u32 = 32;
/// Layers the atlas starts with.
const MIN_LAYERS: u32 = 4;

const COLOR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;
const DATA_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

pub(crate) struct DecalAtlas {
    texture: wgpu::Texture,
    /// The albedo view (sRGB-decoded).
    pub color_view: wgpu::TextureView,
    /// The normal / metallic / roughness view (raw).
    pub data_view: wgpu::TextureView,
    /// Trilinear, anisotropic, clamped to the layer's edge.
    pub sampler: wgpu::Sampler,
    /// Which layer holds each loaded map.
    slots: HashMap<String, u32>,
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
        }
    }

    /// The layer holding `path`, or [`NO_LAYER`] when it has none.
    pub(crate) fn layer(&self, path: &str) -> u32 {
        self.slots.get(path).copied().unwrap_or(NO_LAYER)
    }

    /// Give every map in `paths` (the frame's) a layer, loading the new ones. A path
    /// that fails to load joins `misses`, which the renderer re-checks on disk
    /// (#689). Returns whether the texture was replaced, so group 0 is rebuilt.
    pub(crate) fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        paths: &[&str],
        misses: &mut HashSet<String>,
    ) -> bool {
        let mut fresh: Vec<&str> = Vec::new();
        for &path in paths {
            if !self.slots.contains_key(path) && !misses.contains(path) && !fresh.contains(&path) {
                fresh.push(path);
            }
        }
        if fresh.is_empty() {
            return false;
        }
        let wanted = self.slots.len() + fresh.len();
        let mut replaced = false;
        if wanted > self.capacity() {
            if wanted > MAX_LAYERS as usize {
                self.slots.retain(|path, _| paths.contains(&path.as_str()));
            }
            let needed = (self.slots.len() + fresh.len()) as u32;
            let capacity = needed.next_power_of_two().clamp(MIN_LAYERS, MAX_LAYERS);
            if capacity > self.capacity() as u32 {
                self.grow(device, queue, capacity);
                replaced = true;
            }
        }
        for path in fresh {
            let Some(layer) = self.free_layer() else {
                break;
            };
            match image::open(path) {
                Ok(img) => {
                    upload(queue, &self.texture, layer, img.to_rgba8());
                    self.slots.insert(path.to_string(), layer);
                }
                Err(_) => {
                    misses.insert(path.to_string());
                }
            }
        }
        replaced
    }

    /// Forget the maps `is_stale` names, so the next frame loads them again.
    pub(crate) fn forget(&mut self, is_stale: impl Fn(&str) -> bool) {
        self.slots.retain(|path, _| !is_stale(path));
    }

    fn capacity(&self) -> usize {
        self.texture.depth_or_array_layers() as usize
    }

    /// The lowest layer no map holds.
    fn free_layer(&self) -> Option<u32> {
        let used: HashSet<u32> = self.slots.values().copied().collect();
        (0..self.capacity() as u32).find(|layer| !used.contains(layer))
    }

    /// Replace the array with one of `layers` layers, copying every layer over.
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
        queue.submit(std::iter::once(encoder.finish()));
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
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[DATA_FORMAT],
    })
}

/// The array's sRGB and raw views.
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

/// Resize `img` to a layer, then write it and each halved mip into `layer`.
fn upload(queue: &wgpu::Queue, texture: &wgpu::Texture, layer: u32, img: image::RgbaImage) {
    let mut level = image::imageops::resize(&img, LAYER_SIZE, LAYER_SIZE, FilterType::Triangle);
    for mip in 0..MIPS {
        let side = level.width();
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: mip,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &level,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * side),
                rows_per_image: Some(side),
            },
            wgpu::Extent3d {
                width: side,
                height: side,
                depth_or_array_layers: 1,
            },
        );
        let half = (side / 2).max(1);
        level = image::imageops::resize(&level, half, half, FilterType::Triangle);
    }
}
