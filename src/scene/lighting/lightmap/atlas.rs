//! Packing baked lightmaps into atlas pages (#438), Unity's lightmap atlasing.
//!
//! Every lightmap of a bake lands in one of a few equal-size square pages, so the
//! renderer binds them all as one texture array and each mesh carries only a page
//! index and a scale/offset into it (its `LightmapEntry`). Lightmapped copies of a
//! prop then still draw as one instanced call. Pages are the smallest power of two
//! that holds everything in one page, up to [`MAX_PAGE`]; past that, as many
//! [`MAX_PAGE`] pages as it takes.
//!
//! Each lightmap gets a one-texel ring copied from its own edge, so bilinear
//! filtering at a chart's border never reads a neighbour's light. Packing is a shelf
//! packer over the lightmaps sorted largest first (ties by order), so the same
//! bake always yields the same atlas.
//!
//! A directional bake (#810) packs its direction maps into a second page set with the
//! very same layout, so one page index and scale/offset address both.

use glam::Vec3;

use super::bake::Lightmap;

/// The largest atlas page edge, in texels.
pub const MAX_PAGE: u32 = 1024;
/// The ring around each lightmap, in texels.
const PAD: u32 = 1;
/// The smallest page tried.
const MIN_PAGE: u32 = 32;

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

/// Pack `maps` into pages. No lightmaps, no pages.
pub fn pack(maps: &[Lightmap]) -> LightmapAtlas {
    let sizes: Vec<u32> = maps
        .iter()
        .map(|m| m.size.min(MAX_PAGE - 2 * PAD))
        .collect();
    let mut order: Vec<usize> = (0..maps.len()).collect();
    order.sort_by(|&a, &b| sizes[b].cmp(&sizes[a]).then(a.cmp(&b)));
    let mut page_size = MIN_PAGE;
    let (corners, page_count) = loop {
        let (corners, count) = shelve(&order, &sizes, page_size);
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
        let at = Corner {
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
        placements.push((
            map.entity,
            page,
            [scale, scale, x as f32 / edge, y as f32 / edge],
        ));
    }
    LightmapAtlas {
        page_size,
        pages,
        directions,
        placements,
    }
}

/// Shelf-pack `sizes` (visited in `order`) into `page`-sized pages: each lightmap's
/// `(page, x, y)` corner (inside its ring), and how many pages it took.
fn shelve(order: &[usize], sizes: &[u32], page: u32) -> (Vec<(u32, u32, u32)>, usize) {
    let mut corners = vec![(0, 0, 0); sizes.len()];
    let (mut p, mut x, mut y, mut shelf) = (0u32, 0u32, 0u32, 0u32);
    for &i in order {
        let cell = sizes[i] + 2 * PAD;
        if x + cell > page {
            (x, y, shelf) = (0, y + shelf, 0);
        }
        if y + cell > page {
            (p, x, y, shelf) = (p + 1, 0, 0, 0);
        }
        corners[i] = (p, x + PAD, y + PAD);
        x += cell;
        shelf = shelf.max(cell);
    }
    let count = if sizes.is_empty() { 0 } else { p as usize + 1 };
    (corners, count)
}

/// Where one lightmap lands: its top-left `size`² texels go to `(x, y)` on a
/// `page_size` page.
struct Corner {
    page_size: u32,
    size: u32,
    x: u32,
    y: u32,
}

impl Corner {
    /// Copy `texels` (a `map_size`-wide lightmap) into `page`, then extend its edge
    /// one texel outward into the ring.
    fn blit(&self, page: &mut [Vec3], texels: &[Vec3], map_size: u32) {
        let (size, x, y) = (self.size, self.x, self.y);
        let at = |row: u32, col: u32| {
            texels[(row.min(size - 1) * map_size + col.min(size - 1)) as usize]
        };
        for row in 0..size + 2 * PAD {
            for col in 0..size + 2 * PAD {
                let (src_row, src_col) = (row.saturating_sub(PAD), col.saturating_sub(PAD));
                let (px, py) = (x + col - PAD, y + row - PAD);
                page[(py * self.page_size + px) as usize] = at(src_row, src_col);
            }
        }
    }
}
