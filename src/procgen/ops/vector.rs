//! src/procgen/ops/vector.rs — vector / normal ops.
//!
//! These reshape the *domain* or reinterpret channels: [`mapping`] resamples the
//! input under an affine domain transform (with wrap, so it stays tiling),
//! [`warp`] displaces one input's domain by another's, [`bump_to_normal`] bakes a
//! tangent-space normal map from a height field, [`normal_blend`] lays one normal
//! map over another, and the combine/separate pair shuttle between grayscale
//! channels and an RGB image.

use glam::Vec3;

use super::super::image_buf::{Image, Rgba};
use super::generators::count;

/// Resample `img` under an affine domain transform: translate, rotate (turns) about
/// the domain center, then scale; a `scale > 1` packs more repeats into the canvas.
/// Sampling is bilinear and wraps. With `tiling`, `scale` rounds to whole repeats and
/// `rotation` snaps to quarter turns — the only transforms that map the unit tile's
/// lattice onto itself, so the output still tiles (#392).
pub fn mapping(
    img: &Image,
    scale: [f32; 2],
    rotation: f32,
    translation: [f32; 2],
    tiling: bool,
) -> Image {
    let [su, sv] = scale.map(|s| s.abs().max(1e-4));
    let (sx, sy, (ca, sa)) = if tiling {
        (count(su), count(sv), quarter_turn(rotation))
    } else {
        let ang = rotation * std::f32::consts::TAU;
        (su, sv, (ang.cos(), ang.sin()))
    };
    Image::fill_uv(img.resolution(), |u, v| {
        // Center the domain so rotation/scale pivot at (0.5, 0.5).
        let (cx, cy) = (u - 0.5, v - 0.5);
        let rx = cx * ca - cy * sa;
        let ry = cx * sa + cy * ca;
        img.sample_bilinear_wrapped(
            rx * sx + 0.5 + translation[0],
            ry * sy + 0.5 + translation[1],
        )
    })
}

/// Domain warp: sample `a` at each point displaced by `by`'s red/green (centred on
/// 0.5, so mid-grey moves nothing) times `strength` in tile units. Both reads are
/// bilinear and wrapped, so two tiling inputs give a tiling, resolution-independent
/// result (#406). A grayscale `by` (R = G) can only push along the diagonal: feed it
/// `combine_rgb` of two independent noises for an isotropic warp.
pub fn warp(a: &Image, by: &Image, strength: f32) -> Image {
    Image::fill_uv(a.resolution(), |u, v| {
        let d = by.sample_bilinear_wrapped(u, v);
        a.sample_bilinear_wrapped(u + strength * (d[0] - 0.5), v + strength * (d[1] - 0.5))
    })
}

/// `(cos, sin)` of `turns` snapped to the nearest quarter turn — exact, not via
/// `f32` trig, so a snapped rotation adds no rounding drift at the wrap.
fn quarter_turn(turns: f32) -> (f32, f32) {
    match ((turns * 4.0).round() as i64).rem_euclid(4) {
        0 => (1.0, 0.0),
        1 => (0.0, 1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    }
}

/// Bake a tangent-space normal map from `img`'s red channel as a height field. Uses
/// central differences (wrapped, so the normals tile) converted to **tile units** —
/// the per-pixel delta times the resolution — so `strength` means the same slope at
/// any resolution (#394). Packs the unit normal into `[0, 1]` RGB the renderer's
/// `2*n - 1` decode expects, with B≈1 for a flat surface.
pub fn bump_to_normal(img: &Image, strength: f32) -> Image {
    let res = img.resolution();
    let h = |x: i64, y: i64| img.get_wrapped(x, y)[0];
    // Central difference spans two pixels = 2/res of a tile.
    let k = 0.5 * res as f32 * strength;
    Image::fill_uv(res, |u, v| {
        let x = (u * res as f32) as i64;
        let y = (v * res as f32) as i64;
        let dx = (h(x + 1, y) - h(x - 1, y)) * k;
        let dy = (h(x, y + 1) - h(x, y - 1)) * k;
        // Tangent-space normal of the height surface: (-dx, -dy, 1) normalized.
        let inv = 1.0 / (dx * dx + dy * dy + 1.0).sqrt();
        [
            -dx * inv * 0.5 + 0.5,
            -dy * inv * 0.5 + 0.5,
            inv * 0.5 + 0.5,
            1.0,
        ]
    })
}

/// Lay normal map `detail` over `base` by Reoriented Normal Mapping (Barré-Brisebois
/// & Hill, *Blending in Detail*, 2012): `base` is rotated by the tilt `detail`
/// carries, the result renormalized. Both decode from `[0, 1]` as
/// [`bump_to_normal`] packs them; `strength` scales the detail's XY first (0 leaves
/// `base`). A flat input on either side returns the other. Alpha is `base`'s.
pub fn normal_blend(base: &Image, detail: &Image, strength: f32) -> Image {
    let mut out = base.clone();
    out.for_each_row(|y, row| {
        for (p, d) in row.iter_mut().zip(detail.row(y)) {
            let n = rnm(decode(*p), decode(*d) * Vec3::new(strength, strength, 1.0));
            *p = [n.x * 0.5 + 0.5, n.y * 0.5 + 0.5, n.z * 0.5 + 0.5, p[3]];
        }
    });
    out
}

/// A normal packed in `[0, 1]` RGB as the vector it encodes.
fn decode(p: Rgba) -> Vec3 {
    Vec3::new(p[0], p[1], p[2]) * 2.0 - 1.0
}

/// RNM: `t = base + (0, 0, 1)`, `u = detail × (−1, −1, 1)`, `t·(t·u)/t.z − u`,
/// renormalized. A degenerate result (a base pointing straight down) reads flat.
fn rnm(base: Vec3, detail: Vec3) -> Vec3 {
    let t = base + Vec3::Z;
    let u = detail * Vec3::new(-1.0, -1.0, 1.0);
    let r = t * t.dot(u) / t.z.max(1e-6) - u;
    r.try_normalize().unwrap_or(Vec3::Z)
}

/// Combine up to three single-channel inputs into one RGB image (alpha 1). Each input
/// contributes its red channel to one output channel; missing inputs contribute 0.
pub fn combine_rgb(inputs: &[&Image], resolution: u32) -> Image {
    let mut out = Image::filled(resolution, [0.0, 0.0, 0.0, 1.0]);
    out.for_each_row(|y, row| {
        for (c, src) in inputs.iter().take(3).enumerate() {
            for (p, s) in row.iter_mut().zip(src.row(y)) {
                p[c] = s[0];
            }
        }
    });
    out
}

/// Pull one channel of `img` out as grayscale (written to all RGB). `channel`
/// 0=R, 1=G, 2=B, 3=A (alpha→gray, output alpha forced to 1).
pub fn separate_rgb(mut img: Image, channel: u8) -> Image {
    let c = (channel as usize).min(3);
    img.map_in_place(|p: Rgba| {
        let v = p[c];
        [v, v, v, 1.0]
    });
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::procgen::hash;

    const FLAT: Rgba = [0.5, 0.5, 1.0, 1.0];

    /// A field of tilted normals: the normal map of a noisy height.
    fn normals(seed: u64) -> Image {
        let h = Image::fill_uv(16, |u, v| {
            let x = hash::unit2((u * 16.0) as i64, (v * 16.0) as i64, 0, seed);
            [x, x, x, 1.0]
        });
        bump_to_normal(&h, 0.2)
    }

    fn assert_close(a: &Image, b: &Image) {
        for (p, q) in a.pixels().iter().zip(b.pixels()) {
            assert!((0..4).all(|c| (p[c] - q[c]).abs() < 1e-5), "{p:?} vs {q:?}");
        }
    }

    #[test]
    fn a_flat_normal_on_either_side_returns_the_other() {
        let (n, flat) = (normals(3), Image::filled(16, FLAT));
        assert_close(&normal_blend(&n, &flat, 1.0), &n);
        assert_close(&normal_blend(&flat, &n, 1.0), &n);
    }

    #[test]
    fn zero_strength_keeps_the_base() {
        let (base, detail) = (normals(3), normals(4));
        assert_close(&normal_blend(&base, &detail, 0.0), &base);
    }

    #[test]
    fn the_blend_is_unit_length_and_tilts_further_than_the_base() {
        let tilt = |p: Rgba| ((p[0] - 0.5).powi(2) + (p[1] - 0.5).powi(2)).sqrt();
        // Two copies of one tilt add up: RNM rotates the base by the detail's tilt.
        let n = normals(5);
        let out = normal_blend(&n, &n, 1.0);
        for (p, q) in out.pixels().iter().zip(n.pixels()) {
            assert!((decode(*p).length() - 1.0).abs() < 1e-5);
            if tilt(*q) > 0.05 {
                assert!(tilt(*p) > tilt(*q), "{p:?} vs {q:?}");
            }
        }
    }
}
