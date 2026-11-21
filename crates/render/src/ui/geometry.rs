use glam::{Vec2, Vec4};

use rusty_components::{FillMethod, FillOrigin, ImageComponent, ImageType};

pub(crate) type UnitVertex = (Vec2, Vec2);

pub(crate) const MAX_TILES: f32 = 1024.0;

pub(crate) fn image_triangles(
    image: &ImageComponent,
    size: Vec2,
    tex: Option<Vec2>,
) -> Vec<UnitVertex> {
    let mut out = Vec::new();
    let tex = tex.filter(|t| t.min_element() > 0.0);
    match (image.image_type, tex) {
        (ImageType::Sliced, Some(t)) if image.border != Vec4::ZERO => {
            sliced(&mut out, image.border, size, t)
        }
        (ImageType::Tiled, Some(t)) => tiled(&mut out, size, t),
        (ImageType::Filled, _) => filled(&mut out, image),
        (ImageType::Simple, Some(t)) if image.preserve_aspect => {
            let (lo, hi) = fit_aspect(size, t);
            quad(&mut out, lo, hi, Vec2::ZERO, Vec2::ONE);
        }
        _ => quad(&mut out, Vec2::ZERO, Vec2::ONE, Vec2::ZERO, Vec2::ONE),
    }
    out
}

fn quad(out: &mut Vec<UnitVertex>, lo: Vec2, hi: Vec2, uv_lo: Vec2, uv_hi: Vec2) {
    if hi.x <= lo.x || hi.y <= lo.y {
        return;
    }
    let p = |x: f32, y: f32, u: f32, v: f32| (Vec2::new(x, y), Vec2::new(u, v));
    let bl = p(lo.x, lo.y, uv_lo.x, uv_lo.y);
    let tl = p(lo.x, hi.y, uv_lo.x, uv_hi.y);
    let tr = p(hi.x, hi.y, uv_hi.x, uv_hi.y);
    let br = p(hi.x, lo.y, uv_hi.x, uv_lo.y);
    out.extend([bl, tl, tr, bl, tr, br]);
}

fn fit_aspect(size: Vec2, tex: Vec2) -> (Vec2, Vec2) {
    let scale = (size / tex).min_element();
    let unit = (tex * scale) / size.max(Vec2::splat(f32::EPSILON));
    let lo = (Vec2::ONE - unit) * 0.5;
    (lo, lo + unit)
}

fn sliced(out: &mut Vec<UnitVertex>, border: Vec4, size: Vec2, tex: Vec2) {
    let size = size.max(Vec2::splat(f32::EPSILON));
    let (lo_b, hi_b) = (Vec2::new(border.x, border.y), Vec2::new(border.z, border.w));
    let fit = (size / (lo_b + hi_b).max(Vec2::splat(f32::EPSILON))).min(Vec2::ONE);
    let xs = [
        0.0,
        lo_b.x * fit.x / size.x,
        1.0 - hi_b.x * fit.x / size.x,
        1.0,
    ];
    let ys = [
        0.0,
        lo_b.y * fit.y / size.y,
        1.0 - hi_b.y * fit.y / size.y,
        1.0,
    ];
    let us = [0.0, lo_b.x / tex.x, 1.0 - hi_b.x / tex.x, 1.0];
    let vs = [0.0, lo_b.y / tex.y, 1.0 - hi_b.y / tex.y, 1.0];
    for j in 0..3 {
        for i in 0..3 {
            quad(
                out,
                Vec2::new(xs[i], ys[j]),
                Vec2::new(xs[i + 1], ys[j + 1]),
                Vec2::new(us[i], vs[j]),
                Vec2::new(us[i + 1], vs[j + 1]),
            );
        }
    }
}

fn tiled(out: &mut Vec<UnitVertex>, size: Vec2, tex: Vec2) {
    let count = (size / tex).ceil().max(Vec2::ONE);
    let grow = ((count.x * count.y) / MAX_TILES).sqrt().max(1.0);
    let tile = tex * grow;
    let (nx, ny) = (
        (size.x / tile.x).ceil() as u32,
        (size.y / tile.y).ceil() as u32,
    );
    for j in 0..ny {
        for i in 0..nx {
            let lo = Vec2::new(i as f32, j as f32) * tile;
            let hi = (lo + tile).min(size);
            let uv_hi = (hi - lo) / tile;
            quad(out, lo / size, hi / size, Vec2::ZERO, uv_hi);
        }
    }
}

fn filled(out: &mut Vec<UnitVertex>, image: &ImageComponent) {
    let a = image.fill_amount.clamp(0.0, 1.0);
    let (lo, hi) = match (image.fill_method, image.fill_origin) {
        (FillMethod::Radial360, origin) => return radial(out, a, origin, image.fill_clockwise),
        (FillMethod::Horizontal, FillOrigin::Right | FillOrigin::Top) => {
            (Vec2::new(1.0 - a, 0.0), Vec2::ONE)
        }
        (FillMethod::Horizontal, _) => (Vec2::ZERO, Vec2::new(a, 1.0)),
        (FillMethod::Vertical, FillOrigin::Top | FillOrigin::Right) => {
            (Vec2::new(0.0, 1.0 - a), Vec2::ONE)
        }
        (FillMethod::Vertical, _) => (Vec2::ZERO, Vec2::new(1.0, a)),
    };
    quad(out, lo, hi, lo, hi);
}

fn radial(out: &mut Vec<UnitVertex>, amount: f32, origin: FillOrigin, clockwise: bool) {
    use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};
    if amount >= 1.0 {
        return quad(out, Vec2::ZERO, Vec2::ONE, Vec2::ZERO, Vec2::ONE);
    }
    let sweep = amount * TAU;
    if sweep <= 0.0 {
        return;
    }
    let start = match origin {
        FillOrigin::Bottom => -FRAC_PI_2,
        FillOrigin::Right => 0.0,
        FillOrigin::Top => FRAC_PI_2,
        FillOrigin::Left => PI,
    };
    let dir = if clockwise { -1.0 } else { 1.0 };
    let mut offsets: Vec<f32> = (0..4)
        .map(|k| ((FRAC_PI_4 + k as f32 * FRAC_PI_2 - start) * dir).rem_euclid(TAU))
        .filter(|&d| d > 0.0 && d < sweep)
        .collect();
    offsets.sort_by(f32::total_cmp);
    offsets.insert(0, 0.0);
    offsets.push(sweep);
    let centre = (Vec2::splat(0.5), Vec2::splat(0.5));
    let rim: Vec<Vec2> = offsets.iter().map(|d| rim_point(start + dir * d)).collect();
    for pair in rim.windows(2) {
        out.extend([centre, (pair[0], pair[0]), (pair[1], pair[1])]);
    }
}

fn rim_point(angle: f32) -> Vec2 {
    let d = Vec2::new(angle.cos(), angle.sin());
    Vec2::splat(0.5) + d * (0.5 / d.abs().max_element())
}

#[cfg(test)]
#[path = "geometry_tests.rs"]
mod geometry_tests;
