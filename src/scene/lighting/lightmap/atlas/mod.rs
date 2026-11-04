//! Packing baked lightmaps into atlas pages (#438, #1694), Unity's lightmap
//! atlasing.
//!
//! Every lightmap of a bake lands in one of a few equal-size square pages, so the
//! renderer binds them all as one texture array and each mesh carries only a page
//! index and a scale/offset into it (its `LightmapEntry`). Lightmapped copies of a
//! prop then still draw as one instanced call. Pages are the smallest power of two
//! that holds everything in one page, up to [`MAX_PAGE`]; past that, as many
//! [`MAX_PAGE`] pages as it takes.
//!
//! **The cell.** Each lightmap sits in a square cell: the map, a [`PAD`]-texel ring
//! copied from its own edge, and the cell rounded up to a multiple of [`GRID`], at a
//! corner that is one too. Its edge texels fill the whole cell. So a texel of mip
//! level `L < MIP_LEVELS` (`2^L` base texels wide) never spans two maps, and
//! bilinear filtering at a map's border, at any of those levels, reads only the
//! map's own light. [`MIP_LEVELS`] is capped at what the ring covers: level 2 is 4
//! texels wide, the ring's width; a level 3 texel would straddle two cells.
//!
//! **The grid** is a 4×4 BC block of the last kept level: 16 base texels. A
//! 4-texel grid kept a level-0 block inside one cell but let a level-2 block (16
//! base texels) hold two maps, or a map and the page's empty black, and BC6H then
//! bent a dim map's light by up to 8.6% (`pages_bc_tests`, against 5% elsewhere).
//! On a level-shaped bake (`tests::level_sizes`) the 16-texel grid costs 4% more
//! cell area and no extra page.
//!
//! **The packer** is MaxRects (`rects`) over the cells, largest first (ties by
//! order), each one into the first page with room (a gap an earlier page left is
//! filled before a new page opens). Deterministic: the same bake always yields the
//! same atlas.
//!
//! **The largest map** is [`max_map_edge`] of the page: half a page less its ring,
//! so maps at the cap pack two by two on a page instead of one each (a 512 cap
//! on a 1024 page left 28 of a game's 51 pages a quarter full).
//!
//! A directional bake (#810) packs its direction maps into a second page set with the
//! very same layout, so one page index and scale/offset address both.

mod rects;

use glam::Vec3;

use super::bake::Lightmap;

/// The largest atlas page edge, in texels.
pub const MAX_PAGE: u32 = 2048;
/// The mip levels a page's layout keeps apart (see the module doc): the base and
/// two halvings.
pub const MIP_LEVELS: u32 = 3;
/// The ring around each lightmap, in texels: the width of a texel of the last
/// mip level.
pub const PAD: u32 = 1 << (MIP_LEVELS - 1);
/// The grid cells are aligned to, in texels: a 4×4 BC block of the last mip
/// level, so no block of any kept level spans two cells.
pub const GRID: u32 = 4 << (MIP_LEVELS - 1);
/// The smallest page tried.
const MIN_PAGE: u32 = 32;

/// The largest lightmap edge a `page`-edge atlas takes so that two such maps,
/// with their rings, sit side by side: `page / 2 - 2 * PAD`.
pub const fn max_map_edge(page: u32) -> u32 {
    page / 2 - 2 * PAD
}

/// The packed pages and where each lightmap went.
#[derive(Clone, Debug, PartialEq)]
pub struct LightmapAtlas {
    pub page_size: u32,
    /// Each page's linear texels, row-major, `page_size`² of them.
    pub pages: Vec<Vec<Vec3>>,
    /// The direction pages (#810), one per colour page, packed identically. Empty
    /// unless every lightmap carries directions.
    pub directions: Vec<Vec<Vec3>>,
    /// Per input lightmap, in order: its entity, page, and the scale/offset that maps
    /// its lightmap UV into the page (`uv * st.xy + st.zw`).
    pub placements: Vec<(u32, u32, [f32; 4])>,
}

/// Pack `maps` into pages. No lightmaps, no pages. A map wider than
/// [`max_map_edge`] of [`MAX_PAGE`] keeps only its top-left corner of that size.
pub fn pack(maps: &[Lightmap]) -> LightmapAtlas {
    let sizes: Vec<u32> = maps
        .iter()
        .map(|m| m.size.min(max_map_edge(MAX_PAGE)))
        .collect();
    let mut order: Vec<usize> = (0..maps.len()).collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then(a.cmp(&b)));
    let mut page_size = MIN_PAGE;
    let (corners, page_count) = loop {
        let (corners, count) = place(&order, &sizes, page_size);
        if count <= 1 || page_size >= MAX_PAGE {
            break (corners, count);
        }
        page_size *= 2;
    };
    let mut pages = vec![vec![Vec3::ZERO; (page_size * page_size) as usize]; page_count];
    let directional = !maps.is_empty() && maps.iter().all(|m| !m.directions.is_empty());
    let mut directions = match directional {
        true => pages.clone(),
        false => Vec::new(),
    };
    let mut placements = Vec::with_capacity(maps.len());
    for (i, map) in maps.iter().enumerate() {
        let (page, x, y) = corners[i];
        let at = Cell {
            page_size,
            size: sizes[i],
            x,
            y,
        };
        at.blit(&mut pages[page as usize], &map.texels, map.size);
        if directional {
            at.blit(&mut directions[page as usize], &map.directions, map.size);
        }
        let (scale, edge) = (sizes[i] as f32 / page_size as f32, page_size as f32);
        let (u, v) = ((x + PAD) as f32 / edge, (y + PAD) as f32 / edge);
        placements.push((map.entity, page, [scale, scale, u, v]));
    }
    LightmapAtlas {
        page_size,
        pages,
        directions,
        placements,
    }
}

/// A map's cell edge: the map, its ring, rounded up to the grid.
fn cell_edge(size: u32) -> u32 {
    (size + 2 * PAD).div_ceil(GRID) * GRID
}

/// Pack the cells of `sizes` (visited in `order`) into `page`-edge pages: each
/// map's `(page, x, y)` cell corner, in texels, and how many pages it took.
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

/// Where one lightmap lands: its cell's top-left corner `(x, y)` on a `page_size`
/// page, the map `size`² inside it past the ring.
struct Cell {
    page_size: u32,
    size: u32,
    x: u32,
    y: u32,
}

impl Cell {
    /// Copy `texels` (a `map_size`-wide lightmap) into `page`, extending its edge
    /// outward over the rest of the cell (the ring and the grid's slack).
    fn blit(&self, page: &mut [Vec3], texels: &[Vec3], map_size: u32) {
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
