use glam::Vec3;

use super::super::image_buf::{Image, Rgba, Size};
use super::field::Field;
use super::generators::count;

/// With `tiling`, `scale` rounds to whole repeats and `rotation` snaps to quarter turns, the only
/// transforms that map the unit tile's lattice onto itself.
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
        let (cx, cy) = (u - 0.5, v - 0.5);
        let rx = cx * ca - cy * sa;
        let ry = cx * sa + cy * ca;
        img.sample_bilinear_wrapped(
            rx * sx + 0.5 + translation[0],
            ry * sy + 0.5 + translation[1],
        )
    })
}

pub fn warp(a: &Image, by: &Image, strength: Field) -> Image {
    Image::fill_uv(a.size(), |u, v| {
        let strength = strength.at_pixel(u, v);
        let d = by.sample_bilinear_wrapped(u, v);
        a.sample_bilinear_wrapped(u + strength * (d[0] - 0.5), v + strength * (d[1] - 0.5))
    })
}

/// `(cos, sin)` of `turns` snapped to the nearest quarter turn, exact (no `f32` trig drift).
fn quarter_turn(turns: f32) -> (f32, f32) {
    match ((turns * 4.0).round() as i64).rem_euclid(4) {
        0 => (1.0, 0.0),
        1 => (0.0, 1.0),
        2 => (-1.0, 0.0),
        _ => (0.0, -1.0),
    }
}

/// Slope is in tile units of the longer side on both axes, as Substance's Normal node, so
/// `strength` holds at any resolution. Packed as `n · ½ + ½`, the renderer's `2n − 1` decode.
pub fn bump_to_normal(img: &Image, strength: Field) -> Image {
    let (w, hh) = (img.width() as f32, img.height() as f32);
    let h = |x: i64, y: i64| img.get_wrapped(x, y)[0];
    let longer = img.size().longer() as f32;
    Image::fill_uv(img.size(), |u, v| {
        let x = (u * w) as i64;
        let y = (v * hh) as i64;
        let k = 0.5 * longer * strength.at(y as usize * img.width() as usize + x as usize);
        let dx = (h(x + 1, y) - h(x - 1, y)) * k;
        let dy = (h(x, y + 1) - h(x, y - 1)) * k;
        let inv = 1.0 / (dx * dx + dy * dy + 1.0).sqrt();
        [
            -dx * inv * 0.5 + 0.5,
            -dy * inv * 0.5 + 0.5,
            inv * 0.5 + 0.5,
            1.0,
        ]
    })
}

/// The lowest z [`normal_blend`] writes: below about 0.105, a BC5 copy rebuilding z from 8-bit
/// x and y may find no positive z. Blue byte 143.
pub const MIN_NORMAL_Z: f32 = 0.125;

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

fn decode(p: Rgba) -> Vec3 {
    Vec3::new(p[0], p[1], p[2]) * 2.0 - 1.0
}

fn rnm(base: Vec3, detail: Vec3) -> Vec3 {
    let t = base + Vec3::Z;
    let u = detail * Vec3::new(-1.0, -1.0, 1.0);
    let r = t * t.dot(u) / t.z.max(1e-6) - u;
    r.try_normalize().unwrap_or(Vec3::Z)
}

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
