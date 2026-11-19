//! crates/procgen/src/ops/vector.rs — vector / normal ops.
//!
//! These reshape the *domain* or reinterpret channels: [`mapping`] resamples the
//! input under an affine domain transform (with wrap, so it stays tiling),
//! [`warp`] displaces one input's domain by another's, [`bump_to_normal`] bakes a
//! tangent-space normal map from a height field, [`normal_blend`] lays one normal
//! map over another, and the combine/separate pair shuttle between grayscale
//! channels and an RGB image. A strength is a [`Field`], read per pixel (#1494).

use glam::Vec3;

use super::super::image_buf::{Image, Rgba, Size};
use super::field::Field;
use super::generators::count;

/// Resample `img` under an affine domain transform: each output point reads `img` at
/// that point turned `rotation` turns about the domain center (0.5, 0.5), scaled
/// about it, then offset by `translation` in tile units. As shown (v down), a positive
/// turn turns the content counter-clockwise, a `scale > 1` packs more repeats into
/// the canvas, and a positive translation moves the content towards −U / −V.
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
    Image::fill_uv(img.size(), |u, v| {
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
/// 0.5, so mid-grey moves nothing) times `strength` in tile units: red above 0.5
/// reads further along +U, so the content moves towards −U (green: −V). Both reads are
/// bilinear and wrapped, so two tiling inputs give a tiling, resolution-independent
/// result (#406). A grayscale `by` (R = G) can only push along the diagonal: feed it
/// `combine_rgb` of two independent noises for an isotropic warp. A linked
/// `strength` is read at each output pixel (#1494).
pub fn warp(a: &Image, by: &Image, strength: Field) -> Image {
    Image::fill_uv(a.size(), |u, v| {
        let strength = strength.at_pixel(u, v);
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
/// central differences (wrapped, so the normals tile) converted to **tile units** of
/// the longer side — the per-pixel delta times that side's pixel count — so
/// `strength` means the same slope at any resolution (#394). Both axes share that
/// unit (#1493): a trim sheet is mapped at an even texel density, so the same height
/// step between neighbours slopes the same across and down, as Substance's Normal
/// node does. Packs the unit normal into `[0, 1]` RGB the renderer's `2*n - 1`
/// decode expects, with B≈1 for a flat surface.
pub fn bump_to_normal(img: &Image, strength: Field) -> Image {
    let (w, hh) = (img.width() as f32, img.height() as f32);
    let h = |x: i64, y: i64| img.get_wrapped(x, y)[0];
    let longer = img.size().longer() as f32;
    Image::fill_uv(img.size(), |u, v| {
        let x = (u * w) as i64;
        let y = (v * hh) as i64;
        // Central difference spans two pixels = 2/n of the longer side's tile.
        let k = 0.5 * longer * strength.at(y as usize * img.width() as usize + x as usize);
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

/// The lowest z [`normal_blend`] writes: a tangent-space normal stays above the
/// surface, and far enough above it that a BC5 copy, which keeps only x and y at 8
/// bits per endpoint and rebuilds `z = sqrt(1 - x² - y²)`, still rebuilds a positive
/// z. At z = 1/8, x and y rounded by half an 8-bit step each (√2/255 off in length)
/// still leave z ≈ 0.07; the bare limit is z ≈ 0.105. Blue byte 143.
pub const MIN_NORMAL_Z: f32 = 0.125;

/// Lay normal map `detail` over `base` by Reoriented Normal Mapping (Barré-Brisebois
/// & Hill, *Blending in Detail*, 2012): `base` is rotated by the tilt `detail`
/// carries, the result renormalized. Both decode from `[0, 1]` as
/// [`bump_to_normal`] packs them; `strength` scales the detail's XY first (0 leaves
/// `base`). A flat input on either side returns the other. Alpha is `base`'s. The
/// floor then holds on every output texel (`upper_hemisphere`): one under
/// [`MIN_NORMAL_Z`] is lifted to it, keeping its direction across the surface,
/// whatever the detail or `strength`, so a base texel already under the floor is
/// lifted even with a flat detail or `strength` 0. A texel at or above it is RNM's
/// own.
pub fn normal_blend(base: &Image, detail: &Image, strength: Field) -> Image {
    let mut out = base.clone();
    let w = base.width() as usize;
    out.for_each_row(|y, row| {
        for (x, (p, d)) in row.iter_mut().zip(detail.row(y)).enumerate() {
            let s = strength.at(y * w + x);
            let n = upper_hemisphere(rnm(decode(*p), decode(*d) * Vec3::new(s, s, 1.0)));
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

/// `n` (unit length) with z at least [`MIN_NORMAL_Z`]: below it, z is set to the
/// floor and x, y scaled to keep unit length, so the normal leans the same way
/// across the surface, as far as the floor lets it. One with no x, y (straight down)
/// reads flat. At or above the floor `n` is returned untouched, bit for bit.
fn upper_hemisphere(n: Vec3) -> Vec3 {
    if n.z >= MIN_NORMAL_Z {
        return n;
    }
    let xy = n.truncate();
    let len = xy.length();
    if len <= 1e-6 {
        return Vec3::Z;
    }
    let side = (1.0 - MIN_NORMAL_Z * MIN_NORMAL_Z).sqrt();
    (xy * (side / len)).extend(MIN_NORMAL_Z)
}

/// Combine up to four single-channel inputs into one RGBA image. Each input
/// contributes its red channel to one output channel, R, G, B then alpha (#2002); a
/// missing colour input contributes 0, a missing alpha 1.
pub fn combine_rgb(inputs: &[&Image], size: Size) -> Image {
    let mut out = Image::filled(size, [0.0, 0.0, 0.0, 1.0]);
    out.for_each_row(|y, row| {
        for (c, src) in inputs.iter().take(4).enumerate() {
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
#[path = "vector_tests.rs"]
mod tests;
