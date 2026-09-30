//! src/ui/layout/group.rs — row and column layout groups (#421).
//!
//! Unity's `HorizontalOrVerticalLayoutGroup`, ported axis by axis. Along the
//! group's **main** axis (x for a row, y for a column) children sit one after
//! another: each gets its min size, then — as the rect grows — a lerp toward its
//! preferred size, then a share of any surplus by flexible weight; with no
//! flexible child the block is aligned by `child_alignment`. Along the **cross**
//! axis each child fills the inner rect, clamped between its min and (when not
//! flexible) preferred size, and is aligned inside it. A group that does not
//! control an axis keeps each child's own size there and only positions it.
//!
//! Positions here run from the rect's **left / top** edge (Unity's frame);
//! [`placements`] converts them to the y-up rect frame the walk uses. The
//! horizontal pass runs first so the vertical one can read each child's width.

use glam::{Vec2, Vec4};

use super::grid;
use super::sizes::{element_sizes, layout_children, own_size, Sizes};
use crate::components::{LayoutGroupComponent, LayoutKind};
use crate::ecs::World;

/// A child's `(position from the leading edge, size)` along one axis.
type Slot = (f32, f32);

/// Where the group on `id` puts each of its layout children inside `rect`
/// (`(min, size)`, y-up): `(child, (min, size))` in hierarchy order. Empty when
/// `id` has no group.
pub(super) fn placements(world: &World, id: u32, rect: (Vec2, Vec2)) -> Vec<(u32, (Vec2, Vec2))> {
    let Some(g) = world.layout_group(id) else {
        return Vec::new();
    };
    let children = layout_children(world, id);
    let (min, size) = rect;
    let cells = match g.kind {
        LayoutKind::Grid => grid::place(&g, children.len(), size),
        LayoutKind::Horizontal | LayoutKind::Vertical => linear_place(world, &g, &children, size),
    };
    children
        .into_iter()
        .zip(cells)
        .map(|(c, (pos, sz))| {
            let y = min.y + size.y - pos.y - sz.y;
            (c, (Vec2::new(min.x + pos.x, y), sz))
        })
        .collect()
}

/// The group's own layout sizes along `axis` (its width `width` for heights).
pub(super) fn group_sizes(
    world: &World,
    id: u32,
    g: &LayoutGroupComponent,
    axis: usize,
    width: f32,
    depth: u32,
) -> Sizes {
    let children = layout_children(world, id);
    if g.kind == LayoutKind::Grid {
        return grid::sizes(g, children.len(), axis, width);
    }
    let widths = if axis == 1 {
        slots(world, g, &children, 0, width, &[], depth)
            .into_iter()
            .map(|s| s.1)
            .collect()
    } else {
        Vec::new()
    };
    let entries = child_entries(world, g, &children, axis, &widths, depth);
    totals(g, axis, &entries)
}

/// Lay a row or column out in a `size` rect: widths first, then heights.
fn linear_place(
    world: &World,
    g: &LayoutGroupComponent,
    children: &[u32],
    size: Vec2,
) -> Vec<(Vec2, Vec2)> {
    let xs = slots(world, g, children, 0, size.x, &[], 0);
    let widths: Vec<f32> = xs.iter().map(|s| s.1).collect();
    let ys = slots(world, g, children, 1, size.y, &widths, 0);
    xs.into_iter()
        .zip(ys)
        .map(|(x, y)| (Vec2::new(x.0, y.0), Vec2::new(x.1, y.1)))
        .collect()
}

/// Each child's sizes along `axis` as this group sees them: its layout sizes when
/// the group controls the axis, else its own fixed size; force-expand makes it at
/// least flexible 1.
fn child_entries(
    world: &World,
    g: &LayoutGroupComponent,
    children: &[u32],
    axis: usize,
    widths: &[f32],
    depth: u32,
) -> Vec<Sizes> {
    let entry = |(i, &c): (usize, &u32)| {
        let w = widths.get(i).copied().unwrap_or(0.0);
        let mut s = if g.controls(axis) {
            element_sizes(world, c, axis, w, depth)
        } else {
            Sizes::fixed(own_size(world, c, axis, w, depth))
        };
        if g.force_expands(axis) {
            s.flexible = s.flexible.max(1.0);
        }
        s
    };
    children.iter().enumerate().map(entry).collect()
}

/// The group's totals from its children's entries (Unity's `CalcAlongAxis`).
fn totals(g: &LayoutGroupComponent, axis: usize, entries: &[Sizes]) -> Sizes {
    let pad = padding(g.padding, axis).1;
    let mut t = Sizes::fixed(pad);
    if is_main(g, axis) {
        let sp = g.spacing[axis];
        for e in entries {
            t.min += e.min + sp;
            t.preferred += e.preferred + sp;
            t.flexible += e.flexible;
        }
        if !entries.is_empty() {
            t.min -= sp;
            t.preferred -= sp;
        }
    } else {
        for e in entries {
            t.min = t.min.max(e.min + pad);
            t.preferred = t.preferred.max(e.preferred + pad);
            t.flexible = t.flexible.max(e.flexible);
        }
    }
    t.preferred = t.preferred.max(t.min);
    t
}

/// Every child's slot along `axis` in a rect `size` long (Unity's
/// `SetChildrenAlongAxis`).
fn slots(
    world: &World,
    g: &LayoutGroupComponent,
    children: &[u32],
    axis: usize,
    size: f32,
    widths: &[f32],
    depth: u32,
) -> Vec<Slot> {
    let entries = child_entries(world, g, children, axis, widths, depth);
    let align = alignment(g, axis);
    let place = |pos: f32, space: f32, e: &Sizes| -> Slot {
        if g.controls(axis) {
            (pos, space)
        } else {
            (pos + (space - e.preferred) * align, e.preferred)
        }
    };
    let (lead, pad) = padding(g.padding, axis);
    if !is_main(g, axis) {
        let inner = size - pad;
        let cross = |e: &Sizes| {
            let max = if e.flexible > 0.0 { size } else { e.preferred };
            let space = clamp(inner, e.min, max);
            place(start_offset(g, axis, size, space), space, e)
        };
        return entries.iter().map(cross).collect();
    }
    let total = totals(g, axis, &entries);
    let (mut pos, mut per_flex) = (lead, 0.0);
    let surplus = size - total.preferred;
    if surplus > 0.0 {
        if total.flexible == 0.0 {
            pos = start_offset(g, axis, size, total.preferred - pad);
        } else {
            per_flex = surplus / total.flexible;
        }
    }
    let lerp = if total.min == total.preferred {
        0.0
    } else {
        ((size - total.min) / (total.preferred - total.min)).clamp(0.0, 1.0)
    };
    let mut out = Vec::with_capacity(entries.len());
    for e in &entries {
        let space = e.min + (e.preferred - e.min) * lerp + e.flexible * per_flex;
        out.push(place(pos, space, e));
        pos += space + g.spacing[axis];
    }
    out
}

/// Whether `axis` is the group's main (stacking) axis.
fn is_main(g: &LayoutGroupComponent, axis: usize) -> bool {
    (g.kind == LayoutKind::Horizontal) == (axis == 0)
}

/// `(leading inset, total inset)` along `axis`: left for x, top for y (Unity's
/// frame), from the left-bottom-right-top padding.
pub(super) fn padding(p: Vec4, axis: usize) -> (f32, f32) {
    if axis == 0 {
        (p.x, p.x + p.z)
    } else {
        (p.w, p.y + p.w)
    }
}

/// `child_alignment` along `axis`, 0 at the leading edge (left / top) … 1 the far one.
fn alignment(g: &LayoutGroupComponent, axis: usize) -> f32 {
    let f = g.child_alignment.fractions();
    if axis == 0 {
        f.x
    } else {
        1.0 - f.y
    }
}

/// Where a block `required` long (padding excluded) starts inside a `size` rect,
/// aligned by `child_alignment` (Unity's `GetStartOffset`).
pub(super) fn start_offset(g: &LayoutGroupComponent, axis: usize, size: f32, required: f32) -> f32 {
    let (lead, pad) = padding(g.padding, axis);
    lead + (size - (required + pad)) * alignment(g, axis)
}

/// Unity's `Mathf.Clamp`: `min` wins when the bounds cross (never panics).
fn clamp(v: f32, min: f32, max: f32) -> f32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

#[cfg(test)]
#[path = "group_tests.rs"]
mod group_tests;
