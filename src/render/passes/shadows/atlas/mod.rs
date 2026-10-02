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
//! The atlas is redrawn every frame: unlike a cascade's 2D-array layer, a tile is a
//! sub-rectangle, and WebGPU copies depth only as whole subresources, so the
//! cascades' static-bake copy does not carry over as it is.

mod plan;
pub(crate) use plan::{plan, Tile, ATLAS_SIZE, MAX_TILES};

use super::casters::{CasterBuffer, CasterFrame, Sweep};
use super::{ShadowRenderer, LIGHT_SPACE_STRIDE};
use crate::render::lod::LodSelection;

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

/// The atlas texture, the tile array the forward shader reads, and the depth pass's
/// own light matrices and casters.
pub(crate) struct ShadowAtlas {
    _texture: wgpu::Texture,
    /// The whole atlas: the depth pass's target and the forward shader's input.
    pub view: wgpu::TextureView,
    /// [`MAX_TILES`] `ShadowTile`s, fixed-size so the forward group never rebuilds.
    pub tiles: wgpu::Buffer,
    light_space: wgpu::Buffer,
    pub(super) global: wgpu::BindGroup,
    pub(super) casters: CasterBuffer,
    /// This frame's tiles, in the order their light-space matrices were uploaded.
    frame_tiles: Vec<Tile>,
}

impl ShadowAtlas {
    /// The atlas, its buffers, and its depth pass's group 0 over `global_layout`
    /// (light matrix by dynamic offset, and the cuts' game time from `time_buffer`).
    pub(super) fn new(
        device: &wgpu::Device,
        global_layout: &wgpu::BindGroupLayout,
        entity_layout: &wgpu::BindGroupLayout,
        time_buffer: &wgpu::Buffer,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Shadow Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
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
            _texture: texture,
            view,
            tiles,
            light_space,
            global,
            casters: CasterBuffer::new(device, entity_layout),
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
}

impl ShadowRenderer {
    /// Draw every tile's casters into the atlas, at the LOD levels `lod` shows: one
    /// pass that clears it, one viewport per tile. Nothing when no light is shadowed.
    pub(super) fn render_atlas(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        frame: &CasterFrame,
        lod: &LodSelection,
    ) {
        if self.atlas.frame_tiles.is_empty() {
            return;
        }
        let volumes: Vec<_> = self.atlas.frame_tiles.iter().map(|t| t.view_proj).collect();
        let batches = self.prepare_casters(frame, lod, Sweep::Atlas, &volumes);
        let view = &self.atlas.view;
        let mut pass = super::sweeps::depth_pass(encoder, "Shadow Atlas Pass", view, true);
        for (i, (tile, batches)) in self.atlas.frame_tiles.iter().zip(&batches).enumerate() {
            if batches.is_empty() {
                continue;
            }
            let [x, y] = tile.origin;
            let size = tile.size as f32;
            pass.set_viewport(x as f32, y as f32, size, size, 0.0, 1.0);
            pass.set_scissor_rect(x, y, tile.size, tile.size);
            self.draw_casters(&mut pass, frame, batches, Sweep::Atlas, i);
        }
    }
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
mod contact_tests;
