//! MaxRects (Jylänki's *best short side fit*): free space is the maximal empty rectangles; a gap left by an earlier placement stays available to every later one. Integer arithmetic and a fixed visiting order give the same corners for the same inputs.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Rect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl Rect {
    fn right(&self) -> u32 {
        self.x + self.w
    }

    fn bottom(&self) -> u32 {
        self.y + self.h
    }

    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.right() && o.x < self.right() && self.y < o.bottom() && o.y < self.bottom()
    }

    fn contains(&self, o: &Rect) -> bool {
        self.x <= o.x && self.y <= o.y && o.right() <= self.right() && o.bottom() <= self.bottom()
    }
}

pub(super) struct Page {
    free: Vec<Rect>,
}

impl Page {
    pub fn new(edge: u32) -> Self {
        Self {
            free: vec![Rect {
                x: 0,
                y: 0,
                w: edge,
                h: edge,
            }],
        }
    }

    pub fn place(&mut self, edge: u32) -> Option<(u32, u32)> {
        let best = self
            .free
            .iter()
            .filter(|f| f.w >= edge && f.h >= edge)
            .min_by_key(|f| {
                let (dw, dh) = (f.w - edge, f.h - edge);
                (dw.min(dh), dw.max(dh), f.y, f.x)
            })
            .copied()?;
        let used = Rect {
            x: best.x,
            y: best.y,
            w: edge,
            h: edge,
        };
        self.split(&used);
        Some((used.x, used.y))
    }

    fn split(&mut self, used: &Rect) {
        let mut next = Vec::with_capacity(self.free.len() + 4);
        for f in &self.free {
            if !f.overlaps(used) {
                next.push(*f);
                continue;
            }
            if used.x > f.x {
                next.push(Rect {
                    w: used.x - f.x,
                    ..*f
                });
            }
            if used.right() < f.right() {
                let x = used.right();
                next.push(Rect {
                    x,
                    w: f.right() - x,
                    ..*f
                });
            }
            if used.y > f.y {
                next.push(Rect {
                    h: used.y - f.y,
                    ..*f
                });
            }
            if used.bottom() < f.bottom() {
                let y = used.bottom();
                next.push(Rect {
                    y,
                    h: f.bottom() - y,
                    ..*f
                });
            }
        }
        // Of two equal rectangles the first is kept.
        let kept: Vec<Rect> = next
            .iter()
            .enumerate()
            .filter(|&(i, r)| {
                !next
                    .iter()
                    .enumerate()
                    .any(|(j, o)| j != i && o.contains(r) && (o != r || j < i))
            })
            .map(|(_, r)| *r)
            .collect();
        self.free = kept;
    }
}
