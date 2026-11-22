//! Page levels as the GPU stores them: BC6H irradiance; direction and shadowmask pages stay RGBA8
//! because BC7 turned a direction up to 5° off (a sawtooth in glossy highlights). Levels are made
//! after chart dilation (`raster::fill_rest`), so a mip never averages in the empty background; 2×2
//! box filter in linear light. Mixed directions average to a less directional texel. The RGBM PNG
//! beside each KTX2 page is the fallback for GPUs without BC.

use glam::{Vec3, Vec4};

use rusty_asset::lightmap_page::{self, encode_bc6h, Page};
pub use rusty_scene::lighting::page::{direction_levels, level_count, levels, mask_levels};

/// `None` when `stop` answers true before a level.
pub fn colour_page(
    page: &[Vec3],
    edge: u32,
    threads: usize,
    stop: &dyn Fn() -> bool,
) -> Option<Page> {
    encode_bc6h(edge, &levels(page, edge), threads, stop)
}

pub fn direction_page(page: &[Vec4], edge: u32) -> Page {
    Page {
        format: lightmap_page::PageFormat::Rgba8,
        edge,
        levels: direction_levels(page, edge)
            .into_iter()
            .map(|l| l.concat())
            .collect(),
    }
}
