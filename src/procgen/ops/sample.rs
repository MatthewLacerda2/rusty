//! src/procgen/ops/sample.rs — `sample_at`'s math (#2002): read a map at the
//! coordinates other maps give, filtered by each pixel's footprint.
//!
//! One bilinear tap per pixel aliases the moment a remap shrinks the map (a
//! perspective ground toward its horizon): neighbouring output pixels land texels
//! apart and skip what lies between. So the map is first reduced to a mip chain
//! (each level half the last, its pixel the mean of the 2 × 2 it covers), and each
//! output pixel reads the level whose texel matches its footprint, blended between
//! the two nearest levels (trilinear). The footprint is how far the coordinates move
//! to the pixel's neighbours, taken on each axis as the smaller of the step forward
//! and back, so a seam in the coordinates (a `fract` wrapping from 1 to 0) does not
//! read as one huge footprint and blur the pixels along it.

use super::super::image_buf::{Image, Rgba};

/// Where the coordinates come from.
#[derive(Clone, Copy)]
pub enum Coords<'a> {
    /// One map: red is `u`, green is `v` (what `coordinates` writes).
    Packed(&'a Image),
    /// Two maps: the first's red is `u`, the second's red is `v`.
    Split(&'a Image, &'a Image),
}

impl Coords<'_> {
    /// The coordinates at pixel `(x, y)`, wrapped.
    fn at(self, x: i64, y: i64) -> (f32, f32) {
        match self {
            Coords::Packed(uv) => {
                let p = uv.get_wrapped(x, y);
                (p[0], p[1])
            }
            Coords::Split(u, v) => (u.get_wrapped(x, y)[0], v.get_wrapped(x, y)[0]),
        }
    }
}

/// `src` read at each pixel's coordinates, footprint-filtered through its mip chain.
/// The output has the coordinates' extent; coordinates are in tile units and wrap.
pub fn sample_at(src: &Image, coords: Coords) -> Image {
    let size = match coords {
        Coords::Packed(uv) => uv.size(),
        Coords::Split(u, _) => u.size(),
    };
    let chain = mips(src);
    let (sw, sh) = (src.width() as f32, src.height() as f32);
    let mut out = Image::new(size);
    out.for_each_row(|y, row| {
        let y = y as i64;
        for (x, p) in row.iter_mut().enumerate() {
            let x = x as i64;
            let (u, v) = coords.at(x, y);
            let step = |a: (f32, f32), b: (f32, f32), c: (f32, f32)| {
                let near = |f: fn((f32, f32)) -> f32| (f(b) - f(a)).abs().min((f(c) - f(b)).abs());
                (near(|q| q.0), near(|q| q.1))
            };
            let (dux, dvx) = step(coords.at(x - 1, y), (u, v), coords.at(x + 1, y));
            let (duy, dvy) = step(coords.at(x, y - 1), (u, v), coords.at(x, y + 1));
            let texels = (dux.max(duy) * sw).max(dvx.max(dvy) * sh);
            *p = trilinear(&chain, u, v, lod(texels));
        }
    });
    out
}

/// The mip level whose texel spans `texels` level-0 texels: `log2`, at least 0, and
/// 0 for a footprint that is not a finite number.
fn lod(texels: f32) -> f32 {
    let lod = texels.max(1.0).log2();
    if lod.is_finite() {
        lod
    } else {
        0.0
    }
}

/// `chain` read at `(u, v)` and level `lod`, blended between the two levels around
/// it; past the last level it reads the last.
fn trilinear(chain: &[Image], u: f32, v: f32, lod: f32) -> Rgba {
    let last = chain.len() - 1;
    let lo = (lod.floor() as usize).min(last);
    let a = chain[lo].sample_bilinear_wrapped(u, v);
    let t = lod - lo as f32;
    if lo == last || t <= 0.0 {
        return a;
    }
    let b = chain[lo + 1].sample_bilinear_wrapped(u, v);
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t)
}

/// `img` and each half-size reduction of it down to one texel. A level's texel is
/// the mean of the four bilinear taps at the texel centres of the 2 × 2 it covers:
/// exactly their box mean on an even side, and a wrapped blend on an odd one.
pub fn mips(img: &Image) -> Vec<Image> {
    let mut chain = vec![img.clone()];
    while let Some(prev) = chain.last().filter(|p| p.width() > 1 || p.height() > 1) {
        let (pw, ph) = (prev.width(), prev.height());
        let next = super::super::image_buf::Size::new(pw.div_ceil(2), ph.div_ceil(2));
        let (ox, oy) = (0.5 / pw as f32, 0.5 / ph as f32);
        let (ox, oy) = (if pw > 1 { ox } else { 0.0 }, if ph > 1 { oy } else { 0.0 });
        let level = Image::fill_uv(next, |u, v| {
            let taps = [(-ox, -oy), (ox, -oy), (-ox, oy), (ox, oy)]
                .map(|(dx, dy)| prev.sample_bilinear_wrapped(u + dx, v + dy));
            std::array::from_fn(|c| taps.iter().map(|t| t[c]).sum::<f32>() * 0.25)
        });
        chain.push(level);
    }
    chain
}

#[cfg(test)]
#[path = "sample_tests.rs"]
mod tests;
