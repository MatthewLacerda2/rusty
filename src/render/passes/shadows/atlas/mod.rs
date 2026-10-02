//! The point and spot light shadow atlas (#468), Unity URP's additional-lights way.
//!
//! One depth texture holds every shadowed local light's maps as square tiles: one
//! perspective view for a spotlight, six cube faces for a point light. Each frame
//! [`plan`] picks the lights and their tiles (importance, size, packing), every
//! tile's casters are drawn into its viewport through the cascades' own depth
//! pipelines — so cutout and dissolve casters clip (#648) and skinned ones are posed
//! (#599) here too — and each light's first tile index rides its `LocalLight`
//! record. The forward shader finds the tile through `local_shadow_tile` in
//! `common.wgsl` and filters it with PCF.
//!
//! Static casters are cached per tile, as the cascades cache them per layer (#694,
//! Unity URP's cached additional-light shadows): a second, static atlas keeps each
//! tile's static casters, re-baked only when that tile goes stale ([`cache`]). Each
//! frame every tile is copied from the static atlas into the active one and the
//! dynamic casters are drawn over it ([`sweep`]). WebGPU copies depth only as whole
//! subresources, so the copy and a stale tile's clear are small draws ([`blit`]).

mod blit;
mod cache;
mod plan;
mod sweep;
pub(crate) use plan::{plan, Tile, ATLAS_SIZE, MAX_TILES};

use super::casters::CasterBuffer;
use super::LIGHT_SPACE_STRIDE;
use blit::TileBlit;
use cache::StaticTiles;

/// One tile as the forward shader reads it: `ShadowTile` in `common.wgsl`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ShadowTile {
    pub view_proj: [f32; 16],
    /// The tile in atlas UVs: `xy` top-left corner, `zw` size.
    pub rect: [f32; 4],
    /// `x` world texel size one metre from the light; `yzw` unused.
    pub params: [f32; 4],
}

impl ShadowTile {
    fn new(tile: &Tile) -> Self {
        let atlas = ATLAS_SIZE as f32;
        let [x, y] = tile.origin.map(|v| v as f32 / atlas);
        let size = tile.size as f32 / atlas;
        Self {
            view_proj: tile.view_proj.to_cols_array(),
            rect: [x, y, size, size],
            params: [tile.texel, 0.0, 0.0, 0.0],
        }
    }
}

/// The active and static atlases, the tile array the forward shader reads, and the
/// depth passes' own light matrices and casters.
pub(crate) struct ShadowAtlas {
    /// The active atlas texture; only the tests read it back, everything else draws
    /// and samples it through `view`.
    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) texture: wgpu::Texture,
    /// The whole active atlas: the frame's depth pass's target and the forward
    /// shader's input.
    pub view: wgpu::TextureView,
    _static_texture: wgpu::Texture,
    /// The static atlas: each baked tile's static casters (#694).
    static_view: wgpu::TextureView,
    /// Which tiles the static atlas holds, for which scene.
    static_tiles: StaticTiles,
    blit: TileBlit,
    /// [`MAX_TILES`] `ShadowTile`s, fixed-size so the forward group never rebuilds.
    pub tiles: wgpu::Buffer,
    light_space: wgpu::Buffer,
    pub(super) global: wgpu::BindGroup,
    /// The static bake's and the dynamic pass's casters: both are recorded before
    /// one submit, so each needs its own buffer.
    pub(super) static_casters: CasterBuffer,
    pub(super) dynamic_casters: CasterBuffer,
    /// This frame's tiles, in the order their light-space matrices were uploaded.
    frame_tiles: Vec<Tile>,
}

impl ShadowAtlas {
    /// The atlases, their buffers, the depth passes' group 0 over `global_layout`
    /// (light matrix by dynamic offset, and the cuts' game time from `time_buffer`),
    /// and the tile blits over `blit_shader` (`shadow_atlas.wgsl`).
    pub(super) fn new(
        device: &wgpu::Device,
        [global_layout, entity_layout]: [&wgpu::BindGroupLayout; 2],
        time_buffer: &wgpu::Buffer,
        blit_shader: &wgpu::ShaderModule,
    ) -> Self {
        let texture = depth_texture(device, "Shadow Atlas");
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let static_texture = depth_texture(device, "Shadow Atlas Static");
        let static_view = static_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let blit = TileBlit::new(device, blit_shader, &static_view);
        let buffer = |label, size, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size,
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        let tile_bytes = std::mem::size_of::<ShadowTile>() as u64;
        let tiles = buffer(
            "Shadow Atlas Tiles",
            tile_bytes * MAX_TILES as u64,
            wgpu::BufferUsages::STORAGE,
        );
        let light_space = buffer(
            "Shadow Atlas Light Space",
            LIGHT_SPACE_STRIDE * MAX_TILES as u64,
            wgpu::BufferUsages::UNIFORM,
        );
        let global = super::setup::global_group(device, global_layout, &light_space, time_buffer);
        Self {
            texture,
            view,
            _static_texture: static_texture,
            static_view,
            static_tiles: StaticTiles::default(),
            blit,
            tiles,
            light_space,
            global,
            static_casters: CasterBuffer::new(device, entity_layout),
            dynamic_casters: CasterBuffer::new(device, entity_layout),
            frame_tiles: Vec::new(),
        }
    }

    /// Adopt this frame's tiles: upload what the depth pass and the shader read.
    pub(crate) fn update(&mut self, queue: &wgpu::Queue, tiles: Vec<Tile>) {
        let gpu: Vec<ShadowTile> = tiles.iter().map(ShadowTile::new).collect();
        queue.write_buffer(&self.tiles, 0, bytemuck::cast_slice(&gpu));
        for (i, tile) in tiles.iter().enumerate() {
            let matrix = tile.view_proj.to_cols_array();
            let offset = i as u64 * LIGHT_SPACE_STRIDE;
            queue.write_buffer(&self.light_space, offset, bytemuck::bytes_of(&matrix));
        }
        self.frame_tiles = tiles;
    }

    /// Forget every tile's static bake, so each re-bakes on its next frame.
    pub(super) fn invalidate(&mut self) {
        self.static_tiles.clear();
    }
}

/// A 2048² depth atlas: drawn into, sampled, and copied out by the tests' readback.
fn depth_texture(device: &wgpu::Device, label: &str) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some(label),
        size: wgpu::Extent3d {
            width: ATLAS_SIZE,
            height: ATLAS_SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[cfg(test)]
#[path = "clip_tests.rs"]
mod clip_tests;
#[cfg(test)]
#[path = "gpu_tests.rs"]
mod gpu_tests;
#[cfg(test)]
#[path = "plan_tests.rs"]
mod plan_tests;
#[cfg(test)]
#[path = "static_tests.rs"]
mod static_tests;

#[cfg(test)]
mod contact_tests;
