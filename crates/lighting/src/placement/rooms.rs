use std::collections::VecDeque;

use super::islands::absorb_islands;
use rusty_scene::lighting::floor::FloorGrid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoomParams {
    pub doorway: f32,
    pub step: f32,
}

impl Default for RoomParams {
    fn default() -> Self {
        Self {
            doorway: 2.5,
            step: 0.5,
        }
    }
}

const AXIS: u32 = 3;
const DIAGONAL: u32 = 4;

pub fn find_rooms(grid: &FloorGrid, params: &RoomParams) -> (Vec<Option<u32>>, u32) {
    let clearance = clearance(grid, params.step);
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

pub(super) fn cell_at(grid: &FloorGrid, x: usize, z: usize, dx: isize, dz: isize) -> Option<usize> {
    let nx = x.checked_add_signed(dx).filter(|&v| v < grid.width)?;
    let nz = z.checked_add_signed(dz).filter(|&v| v < grid.depth)?;
    Some(nz * grid.width + nx)
}

fn joined(grid: &FloorGrid, a: usize, b: usize, step: f32) -> bool {
    match (grid.cells[a], grid.cells[b]) {
        (Some(p), Some(q)) => (p.floor - q.floor).abs() <= step,
        _ => false,
    }
}

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
