//! crates/ui/src/layout/group.rs — row and column layout groups.

use glam::{Vec2, Vec4};

use super::grid;
use super::native::TextureSizes;
use super::sizes::{element_sizes, layout_children, own_size, Sizes};
use rusty_components::{LayoutGroupComponent, LayoutKind};
use rusty_ecs::World;

type Slot = (f32, f32);

struct Linear<'a> {
    world: &'a World,
    textures: &'a TextureSizes,
    g: &'a LayoutGroupComponent,
    children: &'a [u32],
    depth: u32,
}

/// Where the group on `id` puts each of its layout children inside `rect`
pub(super) fn placements(
    world: &World,
    textures: &TextureSizes,
    id: u32,
    rect: (Vec2, Vec2),
) -> Vec<(u32, (Vec2, Vec2))> {
    let Some(g) = world.layout_group(id) else {
        return Vec::new();
    };
    let children = layout_children(world, id);
    let (min, size) = rect;
    let cells = match g.kind {
        LayoutKind::Grid => grid::place(&g, children.len(), size),
        LayoutKind::Horizontal | LayoutKind::Vertical => Linear {
            world,
            textures,
            g: &g,
            children: &children,
            depth: 0,
        }
        .place(size),
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

pub(super) fn group_sizes(
    world: &World,
    textures: &TextureSizes,
    id: u32,
    axis: usize,
    width: f32,
    depth: u32,
) -> Option<Sizes> {
    let g = &world.layout_group(id)?;
    let children = layout_children(world, id);
    if g.kind == LayoutKind::Grid {
        return Some(grid::sizes(g, children.len(), axis, width));
    }
    let group = Linear {
        world,
        textures,
        g,
        children: &children,
        depth,
    };
    let widths = if axis == 1 {
        group.widths(width)
    } else {
        Vec::new()
    };
    Some(totals(g, axis, &group.entries(axis, &widths)))
}

impl Linear<'_> {
    /// Lay the children out in a `size` rect: widths first, then heights.
    fn place(&self, size: Vec2) -> Vec<(Vec2, Vec2)> {
        let xs = self.slots(0, size.x, &[]);
        let widths: Vec<f32> = xs.iter().map(|s| s.1).collect();
        let ys = self.slots(1, size.y, &widths);
        xs.into_iter()
            .zip(ys)
            .map(|(x, y)| (Vec2::new(x.0, y.0), Vec2::new(x.1, y.1)))
            .collect()
    }

    /// Each child's width in a `width`-wide group.
    fn widths(&self, width: f32) -> Vec<f32> {
        self.slots(0, width, &[]).into_iter().map(|s| s.1).collect()
    }

    fn entries(&self, axis: usize, widths: &[f32]) -> Vec<Sizes> {
        let (world, textures, g, depth) = (self.world, self.textures, self.g, self.depth);
        let entry = |(i, &c): (usize, &u32)| {
            let w = widths.get(i).copied().unwrap_or(0.0);
            let mut s = if g.controls(axis) {
                element_sizes(world, textures, c, axis, w, depth)
            } else {
                Sizes::fixed(own_size(world, textures, c, axis, w, depth))
            };
            if g.force_expands(axis) {
                s.flexible = s.flexible.max(1.0);
            }
            s
        };
        self.children.iter().enumerate().map(entry).collect()
    }

    /// Every child's slot along `axis` in a rect `size` long (Unity's
    /// `SetChildrenAlongAxis`).
    fn slots(&self, axis: usize, size: f32, widths: &[f32]) -> Vec<Slot> {
        let g = self.g;
        let entries = self.entries(axis, widths);
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

/// Whether `axis` is the group's main (stacking) axis.
fn is_main(g: &LayoutGroupComponent, axis: usize) -> bool {
    (g.kind == LayoutKind::Horizontal) == (axis == 0)
}

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
