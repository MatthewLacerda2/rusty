//! Packing baked lightmaps into atlas pages (#438, #1694), Unity's lightmap
//! atlasing.
//!
//! Every lightmap of a bake lands in one of a few equal-size square pages, so the
//! renderer binds them all as one texture array and each mesh carries only a page
//! index and a scale/offset into it (its `LightmapEntry`). Lightmapped copies of a
//! prop then still draw as one instanced call. Pages are the smallest power of two
//! that holds everything in one page, up to the bake's page edge (Unity's *Max
//! Lightmap Size*, `BakeSettings::max_lightmap_size`, at most [`MAX_PAGE`]); past
//! that, as many pages of that edge as it takes.
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
//! **The largest map** is [`max_map_edge`] of the page: the whole page less its
//! ring, as in Unity, where a mesh's lightmap follows its area up to Max Lightmap
//! Size (#1704). A map above half the page does not strand the rest of it: MaxRects
//! fills the L beside it with smaller maps (`tests`).
//!
//! A directional bake (#810) packs its direction maps into a second page set with the
//! very same layout, so one page index and scale/offset address both.

mod rects;

use glam::Vec3;

use super::bake::Lightmap;

/// The largest atlas page edge, in texels: Unity's largest Max Lightmap Size.
pub const MAX_PAGE: u32 = 4096;
/// The most pages a bake may make (#1704): the layers of one texture array under
/// `wgpu::Limits::default()`, which the renderer asks its device for
/// (`render::gpu::lightmaps` checks the device's own limit at load, and a test
/// holds the two equal).
pub const MAX_PAGES: usize = 256;
/// The mip levels a page's layout keeps apart (see the module doc): the base and
/// two halvings.
pub const MIP_LEVELS: u32 = 3;
/// The ring around each lightmap, in texels: the width of a texel of the last
/// mip level.
pub const PAD: u32 = 1 << (MIP_LEVELS - 1);
/// The grid cells are aligned to, in texels: a 4×4 BC block of the last mip
/// level, so no block of any kept level spans two cells.
pub const GRID: u32 = 4 << (MIP_LEVELS - 1);
/// The smallest page tried: Unity's smallest Max Lightmap Size.
pub const MIN_PAGE: u32 = 32;

/// The largest lightmap edge a `page`-edge atlas takes: the page less the ring on
/// both sides, so its cell is exactly the page (`page` is a multiple of the 16-texel grid).
/// The one definition of a map's cap (#1704).
pub const fn max_map_edge(page: u32) -> u32 {
    page - 2 * PAD
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

/// Pack `maps` into pages of at most `max_page` texels a side (a power of two in
/// [`MIN_PAGE`]`..=`[`MAX_PAGE`]; anything else is first held there by
/// [`page_edge`]). No lightmaps, no pages. A map wider than [`max_map_edge`] of
/// that page keeps only its top-left corner of that size.
pub fn pack(maps: &[Lightmap], max_page: u32) -> LightmapAtlas {
    let Layout {
        page_size,
        sizes,
        corners,
        page_count,
    } = layout(maps, max_page);
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

/// The page edge and page count [`pack`] would make of `maps`, without drawing
/// a texel: what [`check_page_count`] asks before a page is allocated.
pub fn page_count(maps: &[Lightmap], max_page: u32) -> (u32, usize) {
    let l = layout(maps, max_page);
    (l.page_size, l.page_count)
}

/// Refuse a bake of `maps` at `max_page` that makes more than `limit` pages
/// (the renderer's array holds [`MAX_PAGES`], #1704), naming the count, the limit
/// and what to change. Checked before packing: past it, the pages would not load.
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

/// Where every cell goes, before any texel is drawn.
struct Layout {
    page_size: u32,
    /// Each map's edge, held to the cap.
    sizes: Vec<u32>,
    corners: Vec<(u32, u32, u32)>,
    page_count: usize,
}

/// The smallest page edge up to `max_page` that holds `maps` in one page (else
/// `max_page`), and where each map's cell lands on it.
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

/// `edge` held to a page edge the packer takes: the next power of two, inside
/// [`MIN_PAGE`]`..=`[`MAX_PAGE`]. What a hand-edited Max Lightmap Size becomes.
pub fn page_edge(edge: u32) -> u32 {
    edge.clamp(MIN_PAGE, MAX_PAGE).next_power_of_two()
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
