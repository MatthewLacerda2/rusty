pub mod anisotropic;
mod brick;
mod image;
mod noise;
mod shape;
mod voronoi;
mod wave;

pub use image::image;

use super::super::hash;
use super::super::image_buf::{Rgba, Size};
use super::super::recipe::{
    Brick, Checker, Constant, GradientKind, Noise, Shape, Voronoi, VoronoiOutput, Wave,
};
use super::field::Field;

pub type Sampler<'a> = Box<dyn Fn(f32, f32) -> Rgba + Send + Sync + 'a>;

#[inline]
fn gray(v: f32) -> Rgba {
    [v, v, v, 1.0]
}

#[inline]
pub fn finite(x: f32, fallback: f32) -> f32 {
    if x.is_finite() {
        x
    } else {
        fallback
    }
}

#[inline]
pub fn clamp_or(x: f32, default: f32, lo: f32, hi: f32) -> f32 {
    if x.is_nan() {
        default
    } else {
        x.clamp(lo, hi)
    }
}

pub const MAX_COUNT: f32 = 1_048_576.0;

#[inline]
pub fn count(x: f32) -> f32 {
    if x.is_nan() {
        1.0
    } else {
        x.round().clamp(1.0, MAX_COUNT)
    }
}

#[inline]
fn per_axis(scale: f32, aspect: [f32; 2]) -> [f32; 2] {
    aspect.map(|a| scale * a)
}

pub fn constant(p: &Constant) -> Sampler<'static> {
    let color = p.color;
    Box::new(move |_, _| color)
}

pub fn noise<'a>(p: &Noise, gain: Field<'a>, seed: u64) -> Sampler<'a> {
    let octaves = (p.octaves, p.lacunarity, gain);
    noise::sampler(
        p.kind,
        per_axis(p.scale, p.aspect),
        octaves,
        p.distortion,
        seed,
    )
}

pub fn voronoi<'a>(p: &Voronoi, randomness: Field<'a>, seed: u64) -> Sampler<'a> {
    let modes = (p.output, p.metric);
    let scale = per_axis(p.scale, p.aspect);
    let sampler = voronoi::sampler(scale, modes, randomness, p.smoothness, seed);
    if !(p.grayscale && p.output == VoronoiOutput::Cells) {
        return sampler;
    }
    Box::new(move |u, v| gray(sampler(u, v)[0]))
}

pub fn brick(p: &Brick, seed: u64) -> Sampler<'static> {
    brick::sampler(p.rows, p.cols, p.mortar, p.output, seed)
}

pub fn shape<'a>(p: &Shape, softness: Field<'a>) -> Sampler<'a> {
    shape::sampler(p, softness)
}

pub fn white_noise(size: Size, seed: u64) -> Sampler<'static> {
    let (rx, ry) = (size.width.max(1) as i64, size.height.max(1) as i64);
    let (w, h) = (size.width as f32, size.height as f32);
    Box::new(move |u, v| {
        let x = ((u * w).floor() as i64).rem_euclid(rx);
        let y = ((v * h).floor() as i64).rem_euclid(ry);
        gray(hash::unit2(x, y, 0, seed))
    })
}

pub fn checker(p: &Checker) -> Sampler<'static> {
    let (a, b) = (p.color_a, p.color_b);
    let t = p.tiles.max(1).next_multiple_of(2) as f32;
    Box::new(move |u, v| {
        let cx = (u * t).floor() as i64;
        let cy = (v * t).floor() as i64;
        if (cx + cy).rem_euclid(2) == 0 {
            a
        } else {
            b
        }
    })
}

pub fn gradient(kind: GradientKind) -> Sampler<'static> {
    Box::new(move |u, v| match kind {
        GradientKind::Linear => gray(u),
        GradientKind::LinearTiling => gray(1.0 - (2.0 * u.rem_euclid(1.0) - 1.0).abs()),
        GradientKind::Radial => {
            let (dx, dy) = (u - 0.5, v - 0.5);
            let d = (dx * dx + dy * dy).sqrt() / std::f32::consts::FRAC_1_SQRT_2;
            gray(d.min(1.0))
        }
    })
}

pub fn wave(p: &Wave, seed: u64) -> Sampler<'static> {
    let shape = (p.profile, p.distortion, p.detail);
    wave::sampler(p.kind, p.frequency, shape, seed)
}

#[cfg(test)]
mod axis_tests;
#[cfg(test)]
mod tests;
