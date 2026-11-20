//! The ranges a [`ClusterPool`](super::pool::ClusterPool) hands out of one
//! buffer (#2417): first fit over the holes freed meshes left, else past the
//! end; a freed range joins its neighbours, and one that ends the used span
//! shortens it. Pure CPU bookkeeping, in units of the buffer's elements.

use std::ops::Range;

/// A buffer's used span and the holes inside it, in elements.
#[derive(Debug, Default, PartialEq)]
pub(super) struct Ranges {
    /// Where the used span ends: every range handed out lies below it.
    pub(super) end: u32,
    /// The freed holes below `end`, by start, none touching another.
    holes: Vec<Range<u32>>,
}

impl Ranges {
    /// Take `len` elements: the first hole that holds them, else the end of
    /// the used span, unless that would pass `limit`; `None` then.
    pub(super) fn take(&mut self, len: u32, limit: u32) -> Option<Range<u32>> {
        if let Some(h) = self.holes.iter().position(|h| h.len() as u32 >= len) {
            let hole = &mut self.holes[h];
            let taken = hole.start..hole.start + len;
            hole.start += len;
            if hole.start == hole.end {
                self.holes.remove(h);
            }
            return Some(taken);
        }
        let end = self.end.checked_add(len).filter(|&e| e <= limit)?;
        let taken = self.end..end;
        self.end = end;
        Some(taken)
    }

    /// Give `range` back: it joins the holes beside it, and a hole reaching
    /// the end of the used span is dropped from it.
    pub(super) fn free(&mut self, range: Range<u32>) {
        if range.is_empty() {
            return;
        }
        let at = self.holes.partition_point(|h| h.start < range.start);
        self.holes.insert(at, range);
        if at + 1 < self.holes.len() && self.holes[at].end == self.holes[at + 1].start {
            self.holes[at].end = self.holes.remove(at + 1).end;
        }
        if at > 0 && self.holes[at - 1].end == self.holes[at].start {
            self.holes[at - 1].end = self.holes.remove(at).end;
        }
        if self.holes.last().is_some_and(|h| h.end == self.end) {
            self.end = self.holes.pop().map_or(self.end, |h| h.start);
        }
    }

    /// The elements held by ranges handed out and not freed.
    pub(super) fn used(&self) -> u32 {
        self.end - self.holes.iter().map(|h| h.len() as u32).sum::<u32>()
    }
}

#[cfg(test)]
mod tests {
    use super::Ranges;

    #[test]
    fn a_freed_range_is_reused_and_joins_its_neighbours() {
        let mut r = Ranges::default();
        let a = r.take(10, 100).unwrap();
        let b = r.take(20, 100).unwrap();
        let c = r.take(5, 100).unwrap();
        assert_eq!((a.clone(), b.clone(), c.clone()), (0..10, 10..30, 30..35));
        r.free(b);
        assert_eq!(r.take(15, 100), Some(10..25), "the hole, first fit");
        assert_eq!(r.take(8, 100), Some(35..43), "too big for the hole's rest");
        r.free(a);
        r.free(10..25);
        assert_eq!(r.take(30, 100), Some(0..30), "the joined holes hold it");
        assert_eq!(r.used(), 43 - 5 + 5);
    }

    #[test]
    fn freeing_the_last_range_shortens_the_span_and_the_limit_refuses() {
        let mut r = Ranges::default();
        let a = r.take(60, 100).unwrap();
        let b = r.take(40, 100).unwrap();
        assert_eq!(r.take(1, 100), None, "past the limit");
        r.free(a);
        r.free(b);
        assert_eq!(r, Ranges::default(), "everything freed: an empty pool");
        assert_eq!(r.take(0, 0), Some(0..0), "an empty range always fits");
    }
}
