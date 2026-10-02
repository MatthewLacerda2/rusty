//! src/navigation/bake/columns.rs — the spans of a rectangle of cells, and
//! splicing them into a graph (#456).
//!
//! An incremental rebake builds new columns for just the dirty rectangle and
//! splices them over the old ones. The compact storage (`spans` + `cell_start`)
//! is rebuilt in row-sized blocks: the cells outside the rectangle are copied
//! across in runs, so a splice costs one pass of memory copies, not a bake.

use super::super::{NavSpan, NavigationGraph};
use super::region::CellRect;

/// Row-major columns of some [`CellRect`]: the spans of local cell `i` are
/// `spans[cell_start[i]..cell_start[i + 1]]`.
#[derive(Debug, Default, PartialEq)]
pub(in crate::navigation) struct Columns {
    pub cell_start: Vec<u32>,
    pub spans: Vec<NavSpan>,
}

impl Columns {
    pub fn with_capacity(cells: usize) -> Self {
        let mut cell_start = Vec::with_capacity(cells + 1);
        cell_start.push(0);
        Self {
            cell_start,
            spans: Vec::new(),
        }
    }

    /// Append the next cell's spans, bottom-up.
    pub fn push_column(&mut self, spans: impl IntoIterator<Item = NavSpan>) {
        self.spans.extend(spans);
        self.cell_start.push(self.spans.len() as u32);
    }

    /// The spans of local cell `i`.
    pub fn column(&self, i: usize) -> &[NavSpan] {
        &self.spans[self.cell_start[i] as usize..self.cell_start[i + 1] as usize]
    }
}

impl NavigationGraph {
    /// Replace the spans of `region`'s cells with `cols` (built for `region`).
    pub(in crate::navigation) fn splice(&mut self, region: CellRect, cols: &Columns) {
        let old_start = std::mem::take(&mut self.cell_start);
        let old_spans = std::mem::take(&mut self.spans);
        let mut start = Vec::with_capacity(old_start.len());
        let mut spans = Vec::with_capacity(old_spans.len() + cols.spans.len());
        // Copy old cells `c0..c1` across, re-basing their offsets.
        let copy_old = |c0: usize, c1: usize, start: &mut Vec<u32>, spans: &mut Vec<_>| {
            let base = spans.len() as u32;
            start.extend(old_start[c0..c1].iter().map(|&s| s - old_start[c0] + base));
            spans.extend_from_slice(&old_spans[old_start[c0] as usize..old_start[c1] as usize]);
        };
        let mut cursor = 0;
        let w = region.width() as usize;
        for (row, gz) in (region.z0..=region.z1).enumerate() {
            copy_old(cursor, self.index(region.x0, gz), &mut start, &mut spans);
            for i in row * w..(row + 1) * w {
                start.push(spans.len() as u32);
                spans.extend_from_slice(cols.column(i));
            }
            cursor = self.index(region.x1, gz) + 1;
        }
        copy_old(cursor, old_start.len() - 1, &mut start, &mut spans);
        start.push(spans.len() as u32);
        self.cell_start = start;
        self.spans = spans;
    }

    /// The stored columns of `region`.
    pub(in crate::navigation) fn columns(&self, region: CellRect) -> Columns {
        let mut out = Columns::with_capacity(region.cells());
        for gz in region.z0..=region.z1 {
            for gx in region.x0..=region.x1 {
                out.push_column(self.spans_at(gx, gz).iter().copied());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(y: f32) -> NavSpan {
        NavSpan {
            y,
            ceiling: f32::INFINITY,
        }
    }

    #[test]
    fn splice_replaces_only_the_rectangle() {
        let mut g = NavigationGraph::new(0.0, 3.0, 0.0, 2.0, 1.0); // 4 x 3, one span each
        let region = CellRect {
            x0: 1,
            z0: 1,
            x1: 2,
            z1: 2,
        };
        let mut cols = Columns::with_capacity(region.cells());
        cols.push_column([span(1.0), span(5.0)]);
        cols.push_column([]);
        cols.push_column([span(2.0)]);
        cols.push_column([span(3.0)]);
        g.splice(region, &cols);
        assert_eq!(g.cell_start.len(), 13);
        assert_eq!(g.spans.len(), 12 - 4 + 4);
        let ys = |gx, gz| g.spans_at(gx, gz).iter().map(|s| s.y).collect::<Vec<_>>();
        assert_eq!(ys(1, 1), vec![1.0, 5.0]);
        assert!(ys(2, 1).is_empty());
        assert_eq!(ys(1, 2), vec![2.0]);
        assert_eq!(ys(2, 2), vec![3.0]);
        assert_eq!(ys(0, 1), vec![0.0], "outside the rectangle is untouched");
        assert_eq!(ys(3, 2), vec![0.0]);
        assert_eq!(
            g.columns(region),
            cols,
            "reading back gives what was spliced"
        );
    }
}
