//! Reachable: the **main floor** (largest area that is not an elevated top), every walking area
//! holding one of `starts`, and what those reach through off-mesh links, followed one way. The main
//! floor is always seeded: the only actor may be a sentry on a roof.
//!
//! An **elevated top** lies over at least [`COVERED`] of another area's spans, that area holding at
//! least [`COMPARABLE`] of its own count. Size alone would pick a roof (its walls' tops widen it)
//! as main floor.

use std::collections::VecDeque;

use glam::Vec3;

use super::{NavigationGraph, SpanRef};
use rusty_core::collections::Map;

const CARDINAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
pub const COVERED: f32 = 0.5;
/// A tiny pocket below never lifts a floor.
pub const COMPARABLE: f32 = 0.25;

impl NavigationGraph {
    pub fn reachable_spans(&self, starts: &[Vec3]) -> Vec<bool> {
        let (area, sizes) = self.walk_areas();
        self.reachable_spans_in((&area, &sizes), starts)
    }

    pub fn reachable_spans_in(
        &self,
        (area, sizes): (&[u32], &[usize]),
        starts: &[Vec3],
    ) -> Vec<bool> {
        let mut seed = vec![false; sizes.len()];
        if let Some(main) = self.main_floor(area, sizes) {
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

    fn main_floor(&self, area: &[u32], sizes: &[usize]) -> Option<usize> {
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

    pub fn walk_areas(&self) -> (Vec<u32>, Vec<usize>) {
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
