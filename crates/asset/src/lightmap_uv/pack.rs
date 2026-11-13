//! Packing charts into the unit square (#831): a shelf packer, the simple well-known
//! one. Each chart's bounding rectangle is laid flat (rotated a quarter turn when it
//! stands taller than wide), the rectangles are sorted tallest first and placed left
//! to right in rows `margin` apart, with `margin` round the border too, and the
//! whole layout is scaled uniformly into `[0, 1]²`, keeping texels square.

use std::cmp::Ordering;

use glam::Vec2;

use super::chart::Chart;

/// A chart laid flat, its corners moved so its rectangle starts at the origin.
struct Flat {
    size: Vec2,
    corners: Vec<[Vec2; 3]>,
}

/// Every triangle corner's lightmap UV, indexed `triangle * 3 + corner`, with
/// charts `margin` world units apart before scaling.
pub(super) fn pack(charts: &[Chart], margin: f32) -> Vec<[f32; 2]> {
    let flats: Vec<Flat> = charts.iter().map(lay_flat).collect();
    let (offsets, extent) = shelves(&flats, margin);
    let scale = if extent > 0.0 { 1.0 / extent } else { 1.0 };
    let count: usize = charts.iter().map(|c| c.triangles.len()).sum();
    let mut uvs = vec![[0.0; 2]; count * 3];
    for ((chart, flat), offset) in charts.iter().zip(&flats).zip(offsets) {
        for (&t, tri) in chart.triangles.iter().zip(&flat.corners) {
            for (k, p) in tri.iter().enumerate() {
                let uv = ((*p + offset) * scale).clamp(Vec2::ZERO, Vec2::ONE);
                uvs[t as usize * 3 + k] = uv.to_array();
            }
        }
    }
    uvs
}

/// `chart` moved to the origin, turned a quarter so it is at least as wide as tall.
fn lay_flat(chart: &Chart) -> Flat {
    let (lo, hi) = bounds(&chart.corners);
    let size = hi - lo;
    let turn = size.y > size.x;
    let place = |p: Vec2| {
        let p = p - lo;
        if turn {
            Vec2::new(size.y - p.y, p.x)
        } else {
            p
        }
    };
    Flat {
        size: if turn {
            Vec2::new(size.y, size.x)
        } else {
            size
        },
        corners: chart.corners.iter().map(|tri| tri.map(place)).collect(),
    }
}

fn bounds(corners: &[[Vec2; 3]]) -> (Vec2, Vec2) {
    let mut lo = Vec2::splat(f32::INFINITY);
    let mut hi = Vec2::splat(f32::NEG_INFINITY);
    for p in corners.iter().flatten() {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    if lo.is_finite() && hi.is_finite() {
        (lo, hi)
    } else {
        (Vec2::ZERO, Vec2::ZERO)
    }
}

/// Each chart's offset and the square layout's edge. Rows are as wide as the square
/// root of the total padded area (or the widest chart), so the layout comes out
/// roughly square.
fn shelves(flats: &[Flat], margin: f32) -> (Vec<Vec2>, f32) {
    let mut order: Vec<usize> = (0..flats.len()).collect();
    order.sort_by(|&a, &b| {
        let (sa, sb) = (flats[a].size, flats[b].size);
        desc(sa.y, sb.y).then(desc(sa.x, sb.x)).then(a.cmp(&b))
    });
    let padded = |f: &Flat| f.size + Vec2::splat(margin);
    let area: f32 = flats.iter().map(|f| padded(f).x * padded(f).y).sum();
    let widest = flats.iter().map(|f| padded(f).x).fold(0.0, f32::max);
    let row = area.sqrt().max(widest) + margin;
    let mut offsets = vec![Vec2::ZERO; flats.len()];
    let (mut cursor, mut row_height, mut width) = (Vec2::splat(margin), 0.0f32, 0.0f32);
    for i in order {
        let size = flats[i].size;
        if cursor.x > margin && cursor.x + size.x + margin > row {
            cursor = Vec2::new(margin, cursor.y + row_height + margin);
            row_height = 0.0;
        }
        offsets[i] = cursor;
        cursor.x += size.x + margin;
        width = width.max(cursor.x);
        row_height = row_height.max(size.y);
    }
    let height = cursor.y + row_height + margin;
    (offsets, width.max(height))
}

/// Descending order for sizes; NaN-free by construction (finite bounds).
fn desc(a: f32, b: f32) -> Ordering {
    b.total_cmp(&a)
}
