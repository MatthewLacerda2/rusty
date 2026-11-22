use std::collections::VecDeque;

use rusty_scene::lighting::floor::FloorGrid;

/// Square metres (a 3 × 3 m platform): a smaller island reflects the room around it.
pub const MIN_ISLAND_AREA: f32 = 9.0;
/// World units an island looks for the room around it.
const ISLAND_REACH: f32 = 2.0;

pub fn absorb_islands(grid: &FloorGrid, room: &mut [Option<u32>], first: u32, count: u32) -> u32 {
    let min_cells = (MIN_ISLAND_AREA / (grid.cell * grid.cell)).ceil() as usize;
    let mut areas = vec![Vec::new(); (count - first) as usize];
    for (i, r) in room.iter().enumerate() {
        if let Some(r) = r.filter(|&r| r >= first) {
            areas[(r - first) as usize].push(i);
        }
    }
    let small: Vec<bool> = areas.iter().map(|a| a.len() < min_cells).collect();
    let host = |r: u32| r < first || !small[(r - first) as usize];
    let reach = (ISLAND_REACH / grid.cell).ceil() as usize;
    for (island, _) in areas.iter().zip(&small).filter(|(_, s)| **s) {
        if let Some(h) = nearest_room(grid, room, island, reach, host) {
            island.iter().for_each(|&i| room[i] = Some(h));
        }
    }
    renumber(room, count)
}

fn nearest_room(
    grid: &FloorGrid,
    room: &[Option<u32>],
    island: &[usize],
    reach: usize,
    host: impl Fn(u32) -> bool,
) -> Option<u32> {
    let mut seen = vec![false; room.len()];
    island.iter().for_each(|&i| seen[i] = true);
    let mut ring: VecDeque<usize> = island.iter().copied().collect();
    for _ in 0..reach {
        let mut next = VecDeque::new();
        let mut votes: Vec<(u32, usize)> = Vec::new();
        for i in ring {
            let (x, z) = (i % grid.width, i / grid.width);
            for (dx, dz) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let Some(n) = super::rooms::cell_at(grid, x, z, dx, dz) else {
                    continue;
                };
                if std::mem::replace(&mut seen[n], true) {
                    continue;
                }
                match room[n].filter(|&r| host(r)) {
                    Some(r) => match votes.iter_mut().find(|v| v.0 == r) {
                        Some(v) => v.1 += 1,
                        None => votes.push((r, 1)),
                    },
                    None => next.push_back(n),
                }
            }
        }
        if let Some(&(r, _)) = votes
            .iter()
            .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
        {
            return Some(r);
        }
        ring = next;
    }
    None
}

fn renumber(room: &mut [Option<u32>], count: u32) -> u32 {
    let mut used = vec![false; count as usize];
    room.iter().flatten().for_each(|&r| used[r as usize] = true);
    let mut new = vec![0; count as usize];
    let mut next = 0;
    for (r, &u) in used.iter().enumerate() {
        new[r] = next;
        next += u32::from(u);
    }
    room.iter_mut()
        .flatten()
        .for_each(|r| *r = new[*r as usize]);
    next
}

#[cfg(test)]
#[path = "islands_tests.rs"]
mod islands_tests;
