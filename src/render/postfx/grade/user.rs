//! The volume's Color Lookup (#1441): a user LUT (`asset::lut`, a `.cube` or a
//! Unity strip `.png`) uploaded as a 3D texture the grading bake reads, so the
//! composite still grades a pixel with one fetch. Unity's *Color Lookup* override,
//! applied as URP applies it: last, on the display colour, sRGB-encoded.
//!
//! Uploads are cached by path ([`UserLuts`]) and kept in step with the disk the way
//! textures are (`core::fs::freshness`): a written file is read again, a missing
//! one is retried once it appears, and a malformed one is logged once, naming the
//! file and line, and grades neutral until it is written.

use std::collections::BTreeMap;

use crate::asset::lut::{self, Lut};
use crate::components::PostProcessingComponent;
use crate::core::fs::freshness::Freshness;
use crate::core::fs::written::Stale;

/// A volume's Color Lookup: the LUT file, and how much of it applies (`0..=1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lookup<'a> {
    pub path: &'a str,
    pub contribution: f32,
}

impl<'a> Lookup<'a> {
    /// The lookup `volume` names, if it names a file and any of it applies: at a
    /// contribution of `0` nothing is read, uploaded or logged.
    pub fn of(volume: Option<&'a PostProcessingComponent>) -> Option<Self> {
        let vc = volume.filter(|vc| vc.lut_contribution > 0.0)?;
        let path = vc.lut.as_deref().filter(|p| !p.is_empty())?;
        Some(Self {
            path,
            contribution: vc.lut_contribution,
        })
    }
}

/// One uploaded LUT.
pub struct UserLut {
    /// Unique per upload, so a view's bake follows a re-read file or a new path.
    pub(super) id: u64,
    pub(super) view: wgpu::TextureView,
    pub(super) domain: [[f32; 3]; 2],
}

/// Every LUT a volume has named, by path; `None` records one that failed to load.
#[derive(Default)]
pub struct UserLuts {
    freshness: Freshness,
    loaded: BTreeMap<String, Option<UserLut>>,
    uploads: u64,
}

impl UserLuts {
    /// The upload of the LUT at `path`, reading it on first use or after it was
    /// written ([`refresh`](Self::refresh)); `None` (logged once) when it is
    /// missing or malformed.
    pub fn get(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        path: &str,
    ) -> Option<&UserLut> {
        if !self.loaded.contains_key(path) {
            let upload = match lut::load(std::path::Path::new(path)) {
                Ok(lut) => {
                    self.uploads += 1;
                    Some(upload(device, queue, &lut, self.uploads))
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

    /// Drop what was written, or has appeared, since the last look. Called once
    /// per frame, before any view's bake, like `Renderer::refresh_textures`.
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

/// `lut` as an `Rgba32Float` 3D texture: read with `textureLoad` and filtered by
/// the bake itself, so its values stay exact (an identity LUT stays identity).
fn upload(device: &wgpu::Device, queue: &wgpu::Queue, lut: &Lut, id: u64) -> UserLut {
    let n = lut.size;
    let texture = texture(device, n);
    queue.write_texture(
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

/// The 1³ texture bound while no LUT applies (the bake skips it).
pub(super) fn neutral(device: &wgpu::Device) -> wgpu::TextureView {
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
