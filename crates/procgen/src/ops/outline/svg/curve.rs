use glam::Vec2;

use super::{Fault, Parser};

/// Longest chord a curve or arc up to a tile long is cut into, tiles: half a
/// pixel at 1024, two at 4096. A longer one takes [`MAX_CHORDS`] chords of its
/// How far a chord strays from the curve is about length² × curvature / 8.
const CHORD: f32 = 1.0 / 2048.0;
const MAX_CHORDS: u32 = 2048;

impl Parser<'_> {
    pub(super) fn curve(&mut self, kind: u8, origin: Vec2) -> Result<Option<(Vec2, bool)>, Fault> {
        match kind {
            b'C' | b'S' => {
                let c1 = match kind {
                    b'C' => origin + self.pair()?,
                    _ => self.reflected(true),
                };
                let c2 = origin + self.pair()?;
                let to = origin + self.pair()?;
                self.cubic(c1, c2, to)?;
                Ok(Some((c2, true)))
            }
            b'Q' | b'T' => {
                let c = match kind {
                    b'Q' => origin + self.pair()?,
                    _ => self.reflected(false),
                };
                let to = origin + self.pair()?;
                self.quad(c, to)?;
                Ok(Some((c, false)))
            }
            _ => {
                let radii = self.pair()?;
                let rotation = self.number()?;
                let large = self.flag()?;
                let sweep = self.flag()?;
                let to = origin + self.pair()?;
                self.arc(radii, rotation, (large, sweep), to)?;
                Ok(None)
            }
        }
    }

    fn quad(&mut self, c: Vec2, to: Vec2) -> Result<(), Fault> {
        let p0 = self.pen;
        let n = chords(p0.distance(c) + c.distance(to));
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            self.line(p0 * (u * u) + c * (2.0 * u * t) + to * (t * t))?;
        }
        Ok(())
    }

    fn cubic(&mut self, c1: Vec2, c2: Vec2, to: Vec2) -> Result<(), Fault> {
        let p0 = self.pen;
        let n = chords(p0.distance(c1) + c1.distance(c2) + c2.distance(to));
        for i in 1..=n {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let p = p0 * (u * u * u) + c1 * (3.0 * u * u * t) + c2 * (3.0 * u * t * t);
            self.line(p + to * (t * t * t))?;
        }
        Ok(())
    }

    /// SVG's endpoint arc (its implementation notes, F.6.5): radii scaled up when
    /// they cannot reach, a zero radius a straight line.
    fn arc(
        &mut self,
        radii: Vec2,
        degrees: f32,
        flags: (bool, bool),
        to: Vec2,
    ) -> Result<(), Fault> {
        let from = self.pen;
        let (mut rx, mut ry) = (radii.x.abs(), radii.y.abs());
        if from == to {
            return Ok(());
        }
        if rx == 0.0 || ry == 0.0 {
            return self.line(to);
        }
        let (sin, cos) = degrees.to_radians().sin_cos();
        let half = (from - to) / 2.0;
        let p = Vec2::new(cos * half.x + sin * half.y, -sin * half.x + cos * half.y);
        let lambda = (p.x / rx).powi(2) + (p.y / ry).powi(2);
        if lambda > 1.0 {
            rx *= lambda.sqrt();
            ry *= lambda.sqrt();
        }
        let num = rx * rx * ry * ry - rx * rx * p.y * p.y - ry * ry * p.x * p.x;
        let den = rx * rx * p.y * p.y + ry * ry * p.x * p.x;
        let mut k = (num / den).max(0.0).sqrt();
        if flags.0 == flags.1 {
            k = -k;
        }
        let cp = Vec2::new(k * rx * p.y / ry, -k * ry * p.x / rx);
        let mid = (from + to) / 2.0;
        let centre = Vec2::new(cos * cp.x - sin * cp.y, sin * cp.x + cos * cp.y) + mid;
        let angle = |v: Vec2| v.y.atan2(v.x);
        let start = angle(Vec2::new((p.x - cp.x) / rx, (p.y - cp.y) / ry));
        let end = angle(Vec2::new((-p.x - cp.x) / rx, (-p.y - cp.y) / ry));
        let tau = std::f32::consts::TAU;
        let mut sweep = (end - start).rem_euclid(tau);
        if !flags.1 && sweep > 0.0 {
            sweep -= tau;
        }
        let n = chords(sweep.abs() * rx.max(ry));
        for i in 1..n {
            let t = start + sweep * i as f32 / n as f32;
            let e = Vec2::new(rx * t.cos(), ry * t.sin());
            self.line(centre + Vec2::new(cos * e.x - sin * e.y, sin * e.x + cos * e.y))?;
        }
        self.line(to)
    }
}

fn chords(len: f32) -> u32 {
    let n = (len / CHORD).ceil();
    if n.is_nan() {
        1
    } else {
        (n as u32).clamp(1, MAX_CHORDS)
    }
}
