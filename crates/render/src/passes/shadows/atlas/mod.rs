mod alloc;
mod blit;
mod cache;
mod hysteresis;
mod plan;
mod slots;
mod sweep;
use crate::gpu::ops::CountedQueue as _;
pub(crate) use plan::{plan, AtlasPlan, Tile, ATLAS_SIZE, MAX_TILES};
pub(crate) use slots::{Frame, ShadowSlots};

use super::casters::CasterBuffer;
use super::LIGHT_SPACE_STRIDE;
use blit::TileBlit;
use cache::StaticTiles;

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct ShadowTile {
    pub view_proj: [f32; 16],
    pub rect: [f32; 4],
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

pub(crate) struct ShadowAtlas {
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "only the tests read the atlas texture back")
    )]
    pub(super) texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    _static_texture: wgpu::Texture,
    static_view: wgpu::TextureView,
    static_tiles: StaticTiles,
    clean_tiles: StaticTiles,
    fresh: bool,
    pub(crate) slots: ShadowSlots,
    blit: TileBlit,
    pub tiles: wgpu::Buffer,
    light_space: wgpu::Buffer,
    pub(super) global: wgpu::BindGroup,
    pub(super) static_casters: CasterBuffer,
    pub(super) dynamic_casters: CasterBuffer,
    frame_tiles: Vec<Tile>,
}

impl ShadowAtlas {
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
        let global = super::layouts::global_group(device, global_layout, &light_space, time_buffer);
        Self {
            texture,
            view,
            _static_texture: static_texture,
            static_view,
            static_tiles: StaticTiles::default(),
            clean_tiles: StaticTiles::default(),
            fresh: true,
            slots: ShadowSlots::default(),
            blit,
            tiles,
            light_space,
            global,
            static_casters: CasterBuffer::new(device, entity_layout),
            dynamic_casters: CasterBuffer::new(device, entity_layout),
            frame_tiles: Vec::new(),
        }
    }

    pub(crate) fn update(&mut self, queue: &wgpu::Queue, tiles: Vec<Tile>) {
        if !tiles.is_empty() {
            let gpu: Vec<ShadowTile> = tiles.iter().map(ShadowTile::new).collect();
            queue.counted_write_buffer(&self.tiles, 0, bytemuck::cast_slice(&gpu));
            queue.counted_write_buffer(&self.light_space, 0, &light_space_bytes(&tiles));
        }
        self.frame_tiles = tiles;
    }

    pub(super) fn invalidate(&mut self) {
        self.static_tiles.clear();
        self.clean_tiles.clear();
    }
}

fn light_space_bytes(tiles: &[Tile]) -> Vec<u8> {
    let stride = LIGHT_SPACE_STRIDE as usize;
    let mut bytes = vec![0u8; stride * tiles.len()];
    for (chunk, tile) in bytes.chunks_exact_mut(stride).zip(tiles) {
        let matrix = tile.view_proj.to_cols_array();
        chunk[..64].copy_from_slice(bytemuck::bytes_of(&matrix));
    }
    bytes
}

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
#[path = "gpu_fixture.rs"]
mod gpu_fixture;
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
#[path = "active_tests.rs"]
mod active_tests;
#[cfg(test)]
#[path = "budget_tests.rs"]
mod budget_tests;
#[cfg(test)]
mod contact_tests;
#[cfg(all(test, feature = "dev"))]
mod index_gpu_tests;
#[cfg(test)]
#[path = "persist_tests.rs"]
mod persist_tests;
#[cfg(all(test, feature = "dev"))]
mod pvs_cost_tests;
#[cfg(test)]
mod pvs_gpu_tests;
#[cfg(test)]
#[path = "settle_tests.rs"]
mod settle_tests;
#[cfg(test)]
mod sweep_tests;
