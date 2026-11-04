//! src/navigation/reach.rs — which walkable spans an actor can actually get to (#1737).
//!
//! A bake marks every surface an agent could stand on as a span: the floor, but
//! also wall tops, roof tops and the top of a crate. Most of those are islands no
//! actor ever reaches. Probe placement wants only the reachable floor, so this
//! answers it from the graph's own moves: the level's **main floor** (the largest
//! area that is not an elevated top), always, plus every area joined by walking
//! that holds one of `starts` (where actors begin), plus everything those lead to
//! through off-mesh links, followed one way (a drop off a ledge reaches the floor
//! below; the floor does not reach the ledge). The main floor is seeded even when
//! actors start elsewhere: the only actor in an edit scene may be a sentry on a
//! roof while the player is spawned by a script.
//!
//! An area is an **elevated top** when it lies over at least [`COVERED`] of
//! another area's spans and that area holds at least [`COMPARABLE`] of its own
//! span count: a roof over the room under it, an upper storey reached only by a
//! ladder. A roof is larger than its room (its walls' tops widen it), so size
//! alone picks the roof of an indoor level, and so does asking whether most of its
//! spans are over floor: a 14 m roof on 2 m walls has two thirds of its spans over
//! the walls. A yard has nothing beneath it, and a floor over a small sealed nook
//! is not lifted by it. What the rule still misreads: a main floor over a basement
//! at least [`COMPARABLE`] its size counts as a top (the next largest area wins,
//! which may be a roof), and a roof over rooms smaller than that is a candidate
//! main floor. An actor start adds its own area, never removes one.
//!
//! Pure and deterministic: scan order, ties to the lower span index.

use std::collections::VecDeque;

use glam::Vec3;

use super::{NavigationGraph, SpanRef};
use crate::core::collections::Map;

/// The cardinal grid steps: a diagonal move needs both cardinals it cuts past
/// (`neighbour`), so the cardinals alone join every area the diagonals do.
const CARDINAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
/// The share of another area's spans an area must lie over to be an elevated top.
pub const COVERED: f32 = 0.5;
/// The least size, as a share of an area's own span count, of an area it lies over
/// for that to make it an elevated top: a tiny pocket below never lifts a floor.
pub const COMPARABLE: f32 = 0.25;

impl NavigationGraph {
    /// Per span (by index into [`Self::spans`]), whether an actor can reach it: see
    /// the module docs. All `false` when nothing is walkable.
    pub fn reachable_spans(&self, starts: &[Vec3]) -> Vec<bool> {
        let (area, sizes) = self.walk_areas();
        let mut seed = vec![false; sizes.len()];
        if let Some(main) = self.main_floor(&area, &sizes) {
            seed[main] = true;
        }
        for s in starts.iter().filter_map(|&p| self.snap(p)) {
            seed[area[s.index as usize] as usize] = true;
        }
        let mut reach = vec![false; self.spans.len()];
        let mut queue = VecDeque::new();
        for s in self.span_refs() {
            if seed[area[s.index as usize] as usize] {
                reach[s.index as usize] = true;
                queue.push_back(s);
            }
        }
        while let Some(here) = queue.pop_front() {
            let walks = CARDINAL
                .iter()
                .filter_map(|&(dx, dz)| self.link_to(here, here.gx + dx, here.gz + dz));
            let links: Vec<SpanRef> = self.link_moves(here).map(|m| m.to).collect();
            for next in walks.chain(links) {
                if !std::mem::replace(&mut reach[next.index as usize], true) {
                    queue.push_back(next);
                }
            }
        }
        reach
    }

    /// The largest area that is not an elevated top (see the module docs), else the
    /// largest; ties to the lower area. `None` with no area.
    fn main_floor(&self, area: &[u32], sizes: &[usize]) -> Option<usize> {
        // Per (upper area, lower area), how many of the lower one's spans it covers.
        let mut covers: Map<(u32, u32), usize> = Map::default();
        for gz in 0..self.height {
            for gx in 0..self.width {
                let column = self.span_range(gx, gz);
                for low in column.clone() {
                    let mut over: Vec<u32> = (low + 1..column.end).map(|u| area[u]).collect();
                    over.sort_unstable();
                    over.dedup();
                    for up in over.into_iter().filter(|&u| u != area[low]) {
                        *covers.entry((up, area[low])).or_default() += 1;
                    }
                }
            }
        }
        let mut elevated = vec![false; sizes.len()];
        for (&(up, low), &n) in &covers {
            let (up_size, low_size) = (sizes[up as usize] as f32, sizes[low as usize] as f32);
            elevated[up as usize] |=
                n as f32 >= COVERED * low_size && low_size >= COMPARABLE * up_size;
        }
        let largest = |a: &usize, b: &usize| sizes[*a].cmp(&sizes[*b]).then(b.cmp(a));
        let areas = 0..sizes.len();
        (areas.clone().filter(|&a| !elevated[a]).max_by(largest)).or_else(|| areas.max_by(largest))
    }

    /// Each span's walking area (joined by grid steps alone, which go both ways),
    /// numbered in scan order, and each area's span count.
    fn walk_areas(&self) -> (Vec<u32>, Vec<usize>) {
        let mut area = vec![u32::MAX; self.spans.len()];
        let mut sizes = Vec::new();
        let mut queue = VecDeque::new();
        for start in self.span_refs() {
            if area[start.index as usize] != u32::MAX {
                continue;
            }
            let id = sizes.len() as u32;
            area[start.index as usize] = id;
            queue.push_back(start);
            let mut size = 0;
            while let Some(here) = queue.pop_front() {
                size += 1;
                for &(dx, dz) in &CARDINAL {
                    let Some(next) = self.link_to(here, here.gx + dx, here.gz + dz) else {
                        continue;
                    };
                    if area[next.index as usize] == u32::MAX {
                        area[next.index as usize] = id;
                        queue.push_back(next);
                    }
                }
            }
            sizes.push(size);
        }
        (area, sizes)
    }
}

#[cfg(test)]
#[path = "reach_tests.rs"]
mod reach_tests;
