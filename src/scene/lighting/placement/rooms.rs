//! Finding the rooms of a floor plan (#1584): connected open floor, split where it
//! narrows to a doorway.
//!
//! A morphological watershed, three linear passes over the grid:
//!   1. **Clearance** — each open cell's distance to the nearest wall, void or ledge
//!      (a 3-4 chamfer distance transform). Two cells are joined only when their
//!      floors are within a step of each other, so a ledge is a wall here.
//!   2. **Cores** — the cells farther than half a doorway from every wall. A doorway
//!      (or any passage narrower than [`RoomParams::doorway`]) has no such cell, so
//!      it cuts the open floor into one core per room; each core is a flood fill.
//!   3. **Rooms** — every open cell joins the core it reaches first, growing all of
//!      them at once, so a doorway is shared out between the rooms it joins.
//!
//!   4. **Narrow areas** — connected open floor no core reached (a level of
//!      corridors narrower than a doorway, a ledge) is a room of its own, so no
//!      walkable space is left reflecting the sky; one smaller than
//!      [`MIN_ISLAND_AREA`] (the top of a jump box) joins the room around it.
//!
//! Every step runs in scan order with ties to the lower index, so a level always
//! splits the same way.

use std::collections::VecDeque;

use super::floor::FloorGrid;
use super::islands::{absorb_islands, MIN_ISLAND_AREA};

/// What counts as a room.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomParams {
    /// A passage narrower than this, world units, separates two rooms.
    pub doorway: f32,
    /// The most two neighbouring floors may differ by and still be one floor.
    pub step: f32,
}

impl Default for RoomParams {
    /// A doorway is anything under 2.5 m wide (a door, an arch, a corridor mouth);
    /// a 0.5 m step is a stair, not a ledge.
    fn default() -> Self {
        Self {
            doorway: 2.5,
            step: 0.5,
        }
    }
}

/// Chamfer weights: 3 along an axis, 4 along a diagonal (≈ 3√2).
const AXIS: u32 = 3;
const DIAGONAL: u32 = 4;

/// The room of each cell (`None` for no room), and how many rooms there are,
/// numbered in the order their cores are met in scan order.
pub fn find_rooms(grid: &FloorGrid, params: &RoomParams) -> (Vec<Option<u32>>, u32) {
    let clearance = clearance(grid, params.step);
    // A cell `d` chamfer units from a wall's cell has `d / 3` cells between their
    // centres; its open space reaches half a cell plus the inset past its own.
    let to_wall = |d: u32| (d as f32 / AXIS as f32 - 0.5) * grid.cell + grid.inset;
    let core: Vec<bool> = (clearance.iter())
        .map(|&d| d > 0 && to_wall(d) > params.doorway * 0.5)
        .collect();
    let mut room = vec![None; grid.cells.len()];
    let mut count = 0;
    let mut queue = VecDeque::new();
    for start in 0..room.len() {
        if !core[start] || room[start].is_some() {
            continue;
        }
        room[start] = Some(count);
        let mut fill = VecDeque::from([start]);
        while let Some(i) = fill.pop_front() {
            queue.push_back(i);
            for n in neighbours(grid, i, params.step) {
                if core[n] && room[n].is_none() {
                    room[n] = Some(count);
                    fill.push_back(n);
                }
            }
        }
        count += 1;
    }
    grow(grid, params.step, &mut room, queue);
    let cores = count;
    // What no core reached: each connected area its own room, but an island too
    // small for a probe of its own joins the room around it (`islands.rs`).
    for start in 0..room.len() {
        if grid.cells[start].is_some() && room[start].is_none() {
            room[start] = Some(count);
            grow(grid, params.step, &mut room, VecDeque::from([start]));
            count += 1;
        }
    }
    let count = absorb_islands(grid, &mut room, cores, count);
    (room, count)
}

/// Grow the rooms of the cells in `queue` into every unlabelled cell joined to
/// them, all at once, in queue order.
fn grow(grid: &FloorGrid, step: f32, room: &mut [Option<u32>], mut queue: VecDeque<usize>) {
    while let Some(i) = queue.pop_front() {
        for n in neighbours(grid, i, step) {
            if room[n].is_none() {
                room[n] = room[i];
                queue.push_back(n);
            }
        }
    }
}

/// The open cells joined to cell `i` along an axis: open, and within a `step`.
fn neighbours(grid: &FloorGrid, i: usize, step: f32) -> impl Iterator<Item = usize> + '_ {
    let (x, z) = (i % grid.width, i / grid.width);
    let at = move |dx: isize, dz: isize| {
        let n = cell_at(grid, x, z, dx, dz)?;
        joined(grid, i, n, step).then_some(n)
    };
    [at(-1, 0), at(1, 0), at(0, -1), at(0, 1)]
        .into_iter()
        .flatten()
}

/// The index of the cell `(dx, dz)` from `(x, z)`, if it is on the grid.
pub(super) fn cell_at(grid: &FloorGrid, x: usize, z: usize, dx: isize, dz: isize) -> Option<usize> {
    let nx = x.checked_add_signed(dx).filter(|&v| v < grid.width)?;
    let nz = z.checked_add_signed(dz).filter(|&v| v < grid.depth)?;
    Some(nz * grid.width + nx)
}

/// Whether cells `a` and `b` are one floor: both open, within a `step`.
fn joined(grid: &FloorGrid, a: usize, b: usize, step: f32) -> bool {
    match (grid.cells[a], grid.cells[b]) {
        (Some(p), Some(q)) => (p.floor - q.floor).abs() <= step,
        _ => false,
    }
}

/// Each cell's chamfer distance to the nearest cell it is not joined to (a wall,
/// the void, a ledge or the grid's edge, all at 0); 0 for a closed cell.
fn clearance(grid: &FloorGrid, step: f32) -> Vec<u32> {
    let mut d: Vec<u32> = (grid.cells.iter())
        .map(|c| if c.is_some() { u32::MAX } else { 0 })
        .collect();
    let forward = [
        (-1, 0, AXIS),
        (-1, -1, DIAGONAL),
        (0, -1, AXIS),
        (1, -1, DIAGONAL),
    ];
    let backward = [
        (1, 0, AXIS),
        (1, 1, DIAGONAL),
        (0, 1, AXIS),
        (-1, 1, DIAGONAL),
    ];
    let relax = |i: usize, d: &mut Vec<u32>, steps: &[(isize, isize, u32)]| {
        if d[i] == 0 {
            return;
        }
        let (x, z) = (i % grid.width, i / grid.width);
        for &(dx, dz, w) in steps {
            let via = match cell_at(grid, x, z, dx, dz) {
                Some(n) if joined(grid, i, n, step) => d[n].saturating_add(w),
                _ => w,
            };
            d[i] = d[i].min(via);
        }
    };
    for i in 0..d.len() {
        relax(i, &mut d, &forward);
    }
    for i in (0..d.len()).rev() {
        relax(i, &mut d, &backward);
    }
    d
}

#[cfg(test)]
#[path = "rooms_tests.rs"]
mod rooms_tests;
