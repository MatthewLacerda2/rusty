//! Packs lightmaps into equal-size square pages: the smallest power-of-two edge, up to Max Lightmap
//! Size. A map sits in a cell: itself plus a [`PAD`]-texel ring copied from its edge, rounded up to
//! [`GRID`] (a BC6H block at mip 2; a 4-texel grid bent a dim map's light up to 8.6%), so no mip or
//! bilinear read crosses two maps. Packed largest first.

mod packing;
mod rects;

pub use packing::{packing, AtlasPage, Packing};

use glam::{Vec3, Vec4};

use super::bake::Lightmap;
use rusty_scene::lighting::page::MIP_LEVELS;
use rusty_scene::lighting::settings::{page_edge, MIN_PAGE};

pub const MAX_PAGES: usize = 256;
pub const PAD: u32 = 1 << (MIP_LEVELS - 1);

pub const fn max_map_edge(page: u32) -> u32 {
    page - 2 * PAD
}

pub const GRID: u32 = 4 << (MIP_LEVELS - 1);

#[derive(Clone, Debug, PartialEq)]
pub struct LightmapAtlas {
    pub page_size: u32,
    pub pages: Vec<Vec<Vec3>>,
    pub directions: Vec<Vec<Vec4>>,
    pub masks: Vec<Vec<Vec4>>,
    /// `uv * st.xy + st.zw` maps a lightmap UV into the page.
    pub placements: Vec<(u32, u32, [f32; 4])>,
}

pub fn pack(maps: &[Lightmap], max_page: u32) -> LightmapAtlas {
    let packing = packing(maps, max_page);
    let (mut pages, mut directions, mut masks) = (Vec::new(), Vec::new(), Vec::new());
    for p in 0..packing.page_count {
        let page = packing.page(p, &|i| &maps[i]);
        pages.push(page.texels);
        directions.extend(page.directions);
        masks.extend(page.masks);
    }
    LightmapAtlas {
        page_size: packing.page_size,
        pages,
        directions,
        masks,
        placements: packing.placements(maps),
    }
}

pub fn page_count(maps: &[Lightmap], max_page: u32) -> (u32, usize) {
    let l = layout(maps, max_page);
    (l.page_size, l.page_count)
}

pub fn check_page_count(maps: &[Lightmap], max_page: u32, limit: usize) -> Result<(), String> {
    let (size, count) = page_count(maps, max_page);
    if count <= limit {
        return Ok(());
    }
    Err(format!(
        "the bake needs {count} lightmap pages of {size}², more than the {limit} a lightmap \
         texture array holds: raise Max Lightmap Size (now {size}), or lower texelsPerUnit or \
         a mesh's Scale In Lightmap; the scene keeps its previous lightmaps"
    ))
}

struct Layout {
    page_size: u32,
    sizes: Vec<u32>,
    corners: Vec<(u32, u32, u32)>,
    page_count: usize,
}

fn layout(maps: &[Lightmap], max_page: u32) -> Layout {
    let max_page = page_edge(max_page);
    let sizes: Vec<u32> = maps
        .iter()
        .map(|m| m.size.min(max_map_edge(max_page)))
        .collect();
    let mut order: Vec<usize> = (0..maps.len()).collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then(a.cmp(&b)));
    let mut page_size = MIN_PAGE;
    loop {
        let (corners, page_count) = place(&order, &sizes, page_size);
        if page_count <= 1 || page_size >= max_page {
            return Layout {
                page_size,
                sizes,
                corners,
                page_count,
            };
        }
        page_size *= 2;
    }
}

fn cell_edge(size: u32) -> u32 {
    (size + 2 * PAD).div_ceil(GRID) * GRID
}

fn place(order: &[usize], sizes: &[u32], page: u32) -> (Vec<(u32, u32, u32)>, usize) {
    let mut corners = vec![(0, 0, 0); sizes.len()];
    let mut pages: Vec<rects::Page> = Vec::new();
    for &i in order {
        let cell = cell_edge(sizes[i]) / GRID;
        let spot = pages
            .iter_mut()
            .enumerate()
            .find_map(|(p, free)| free.place(cell).map(|(x, y)| (p, x, y)));
        let (p, x, y) = match spot {
            Some(found) => found,
            None => {
                let mut fresh = rects::Page::new(page / GRID);
                // A cell is never wider than a page: `page` grows until it is.
                let Some((x, y)) = fresh.place(cell) else {
                    return (corners, usize::MAX);
                };
                pages.push(fresh);
                (pages.len() - 1, x, y)
            }
        };
        corners[i] = (p as u32, x * GRID, y * GRID);
    }
    (corners, pages.len())
}

struct Cell {
    page_size: u32,
    size: u32,
    x: u32,
    y: u32,
}

impl Cell {
    fn blit<T: Copy>(&self, page: &mut [T], texels: &[T], map_size: u32) {
        let (size, edge) = (self.size, cell_edge(self.size));
        let at = |row: u32, col: u32| {
            texels[(row.min(size - 1) * map_size + col.min(size - 1)) as usize]
        };
        for row in 0..edge {
            for col in 0..edge {
                let (src_row, src_col) = (row.saturating_sub(PAD), col.saturating_sub(PAD));
                let (px, py) = (self.x + col, self.y + row);
                page[(py * self.page_size + px) as usize] = at(src_row, src_col);
            }
        }
    }
}

#[cfg(test)]
mod tests;
