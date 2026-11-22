//! The footprint is how far the coordinates move to the pixel's neighbours, per axis the smaller of
//! the step forward and back, so a seam (a `fract` wrapping 1 to 0) does not read as one huge
//! footprint.

use super::super::image_buf::{Image, Rgba};

#[derive(Clone, Copy)]
pub enum Coords<'a> {
    Packed(&'a Image),
    Split(&'a Image, &'a Image),
}

impl Coords<'_> {
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
            *p = trilinear(src, &chain, u, v, lod(texels));
        }
    });
    out
}

fn lod(texels: f32) -> f32 {
    let lod = texels.max(1.0).log2();
    if lod.is_finite() {
        lod
    } else {
        0.0
    }
}

fn trilinear(src: &Image, below: &[Image], u: f32, v: f32, lod: f32) -> Rgba {
    let level = |l: usize| if l == 0 { src } else { &below[l - 1] };
    let last = below.len();
    let lo = (lod.floor() as usize).min(last);
    let a = level(lo).sample_bilinear_wrapped(u, v);
    let t = lod - lo as f32;
    if lo == last || t <= 0.0 {
        return a;
    }
    let b = level(lo + 1).sample_bilinear_wrapped(u, v);
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t)
}

/// Each half-size reduction of `img` down to one texel, `img` itself left out (level 0). A level's
/// texel is the mean of the four bilinear taps at the texel centres of the 2 × 2 it
/// covers: exactly their box mean on an even side, and a wrapped blend on an odd one.
pub fn mips(img: &Image) -> Vec<Image> {
    let mut chain: Vec<Image> = Vec::new();
    while let Some(prev) =
        Some(chain.last().unwrap_or(img)).filter(|p| p.width() > 1 || p.height() > 1)
    {
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
