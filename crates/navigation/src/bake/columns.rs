use super::super::{NavSpan, NavigationGraph};
use super::region::CellRect;

/// Row-major columns of some [`CellRect`]: the spans of local cell `i` are
/// `spans[cell_start[i]..cell_start[i + 1]]`.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Columns {
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

    pub fn push_column(&mut self, spans: impl IntoIterator<Item = NavSpan>) {
        self.spans.extend(spans);
        self.cell_start.push(self.spans.len() as u32);
    }

    pub fn column(&self, i: usize) -> &[NavSpan] {
        &self.spans[self.cell_start[i] as usize..self.cell_start[i + 1] as usize]
    }
}

impl NavigationGraph {
    /// Writes `cols` over `region`, tile by tile: a tile `region` misses keeps its ids.
    pub(crate) fn splice(&mut self, region: CellRect, cols: &Columns) {
        let keys: Vec<_> = self.tiles.keys_over(region).collect();
        for key in keys {
            let Some(rect) = self.tiles.rect(key) else {
                continue;
            };
            let side = self.tiles.side;
            let (x0, z0) = (
                key.0 * side - self.tiles.origin.0,
                key.1 * side - self.tiles.origin.1,
            );
            let mut starts = Vec::with_capacity((side * side) as usize + 1);
            let mut spans = Vec::new();
            starts.push(0);
            for gz in z0..z0 + side {
                for gx in x0..x0 + side {
                    if region.contains(gx, gz) {
                        let i = (gz - region.z0) * region.width() + gx - region.x0;
                        spans.extend_from_slice(cols.column(i as usize));
                    } else if rect.contains(gx, gz) {
                        spans.extend_from_slice(self.spans_at(gx, gz));
                    }
                    starts.push(spans.len() as u32);
                }
            }
            self.tiles.write(key, starts, &spans, &mut self.spans);
        }
    }

    /// A test hook: one column's spans replaced, as a bake would write them.
    #[doc(hidden)]
    pub fn set_column(&mut self, gx: i32, gz: i32, spans: &[NavSpan]) {
        let region = CellRect {
            x0: gx,
            z0: gz,
            x1: gx,
            z1: gz,
        };
        let mut cols = Columns::with_capacity(1);
        cols.push_column(spans.iter().copied());
        self.splice(region, &cols);
    }

    pub(crate) fn columns(&self, region: CellRect) -> Columns {
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
            area: 0,
            surface: crate::Surface::FLAT,
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
        assert_eq!(g.span_count(), 12 - 4 + 4);
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
