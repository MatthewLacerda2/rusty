//! src/navigation/reach.rs — which walkable spans an actor can actually get to (#1737).
//!
//! A bake marks every surface an agent could stand on as a span: the floor, but
//! also wall tops, roof tops and the top of a crate. Most of those are islands no
//! actor ever reaches. Probe placement wants only the reachable floor, so this
//! answers it from the graph's own moves: the largest area joined by walking (the
//! level's main floor), plus the areas holding any `starts` (where actors begin),
//! plus everything those lead to through off-mesh links, followed one way (a drop
//! off a ledge reaches the floor below; the floor does not reach the ledge).
//!
//! Pure and deterministic: scan order, ties to the lower span index.

use std::collections::VecDeque;

use glam::Vec3;

use super::{NavigationGraph, SpanRef};

/// The cardinal grid steps: a diagonal move needs both cardinals it cuts past
/// (`neighbour`), so the cardinals alone join every area the diagonals do.
const CARDINAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

impl NavigationGraph {
    /// Per span (by index into [`Self::spans`]), whether an actor can reach it: see
    /// the module docs. All `false` when nothing is walkable.
    pub fn reachable_spans(&self, starts: &[Vec3]) -> Vec<bool> {
        let (area, sizes) = self.walk_areas();
        let mut seed = vec![false; sizes.len()];
        let largest = (0..sizes.len()).max_by(|&a, &b| sizes[a].cmp(&sizes[b]).then(b.cmp(&a)));
        if let Some(l) = largest {
            seed[l] = true;
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
