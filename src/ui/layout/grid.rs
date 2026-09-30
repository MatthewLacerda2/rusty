//! src/ui/layout/grid.rs — the grid layout group (#421).
//!
//! Unity's `GridLayoutGroup`: every child becomes one `cell_size` cell. The column
//! count is fixed (`FixedColumnCount`), derived from a fixed row count
//! (`FixedRowCount`), or as many as fit the rect (`Flexible`). Cells fill rows
//! first (or columns, with `start_vertical`) from `start_corner`, and the whole
//! block is aligned by `child_alignment`. A grid is never flexible, and it ignores
//! the row/column group's control and force-expand flags.

use glam::Vec2;

use super::group::{padding, start_offset};
use super::sizes::Sizes;
use crate::components::{LayoutConstraint, LayoutCorner, LayoutGroupComponent};

/// The grid's min / preferred size along `axis` for `n` cells; `width` is the
/// grid's own width (a flexible grid's row count depends on it).
pub(super) fn sizes(g: &LayoutGroupComponent, n: usize, axis: usize, width: f32) -> Sizes {
    let count = g.constraint_count.max(1) as usize;
    let (min, preferred) = if axis == 0 {
        match g.constraint {
            LayoutConstraint::FixedColumnCount => (count, count),
            LayoutConstraint::FixedRowCount => (n.div_ceil(count), n.div_ceil(count)),
            LayoutConstraint::Flexible => (1, (n as f32).sqrt().ceil() as usize),
        }
    } else {
        let rows = match g.constraint {
            LayoutConstraint::FixedColumnCount => n.div_ceil(count),
            LayoutConstraint::FixedRowCount => count,
            LayoutConstraint::Flexible => n.div_ceil(fitting(g, 0, width)),
        };
        (rows, rows)
    };
    Sizes {
        min: extent(g, axis, min),
        preferred: extent(g, axis, preferred),
        flexible: 0.0,
    }
}

/// Every cell's `(position from the top-left, size)` for `n` children in a
/// `size` rect (Unity's `SetCellsAlongAxis`).
pub(super) fn place(g: &LayoutGroupComponent, n: usize, size: Vec2) -> Vec<(Vec2, Vec2)> {
    let count = g.constraint_count.max(1) as usize;
    let spill = |fixed: usize| if n > fixed { n.div_ceil(fixed) } else { 1 };
    let (cx, cy) = match g.constraint {
        LayoutConstraint::FixedColumnCount => (count, spill(count)),
        LayoutConstraint::FixedRowCount => (spill(count), count),
        LayoutConstraint::Flexible => (fitting(g, 0, size.x), fitting(g, 1, size.y)),
    };
    let (per_main, ax, ay) = if g.start_vertical {
        let ay = cy.clamp(1, n.max(1));
        (cy, cx.clamp(1, n.div_ceil(cy).max(1)), ay)
    } else {
        let ax = cx.clamp(1, n.max(1));
        (cx, ax, cy.clamp(1, n.div_ceil(cx).max(1)))
    };
    let required = Vec2::new(span(g, 0, ax), span(g, 1, ay));
    let start = Vec2::new(
        start_offset(g, 0, size.x, required.x),
        start_offset(g, 1, size.y, required.y),
    );
    let flip_x = matches!(
        g.start_corner,
        LayoutCorner::UpperRight | LayoutCorner::LowerRight
    );
    let flip_y = matches!(
        g.start_corner,
        LayoutCorner::LowerLeft | LayoutCorner::LowerRight
    );
    let step = g.cell_size + g.spacing;
    (0..n)
        .map(|i| {
            let (a, b) = ((i % per_main) as f32, (i / per_main) as f32);
            let (mut px, mut py) = if g.start_vertical { (b, a) } else { (a, b) };
            if flip_x {
                px = ax as f32 - 1.0 - px;
            }
            if flip_y {
                py = ay as f32 - 1.0 - py;
            }
            (start + step * Vec2::new(px, py), g.cell_size)
        })
        .collect()
}

/// How many cells fit along `axis` in `extent` (at least 1; unbounded when a
/// cell plus its spacing takes no room).
fn fitting(g: &LayoutGroupComponent, axis: usize, extent: f32) -> usize {
    let step = g.cell_size[axis] + g.spacing[axis];
    if step <= 0.0 {
        return usize::MAX;
    }
    let pad = padding(g.padding, axis).1;
    (((extent - pad + g.spacing[axis] + 0.001) / step).floor() as usize).max(1)
}

/// The length `cells` cells take along `axis`, spacing included, padding not.
fn span(g: &LayoutGroupComponent, axis: usize, cells: usize) -> f32 {
    if cells == 0 {
        return 0.0;
    }
    cells as f32 * g.cell_size[axis] + (cells - 1) as f32 * g.spacing[axis]
}

/// The grid's length along `axis` for `cells` cells, padding included.
fn extent(g: &LayoutGroupComponent, axis: usize, cells: usize) -> f32 {
    padding(g.padding, axis).1 + span(g, axis, cells)
}

#[cfg(test)]
#[path = "grid_tests.rs"]
mod grid_tests;
