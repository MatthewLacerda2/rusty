//! What one material's bakes must show: physically plausible base colour (Substance's
//! *PBR Validate* range), roughness, normals and height that use their range, and
//! maps that tile.

use rusty::procgen::Image;

/// The PBR and range checks on one set, in `OUTPUTS` order.
pub fn maps_fail(maps: &[Image], metal: Option<&Image>) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    let px = |img: &Image, c: usize| img.pixels().iter().map(|p| p[c]).collect::<Vec<f32>>();
    let (base, normal, rough, height) = (&maps[0], &maps[1], &maps[2], &maps[3]);
    let is_metal = |i: usize| metal.is_some_and(|m| m.pixels()[i][0] > 0.5);
    let (mut dielectric, mut metallic) = (Vec::new(), Vec::new());
    for (i, p) in base.pixels().iter().enumerate() {
        let y = 0.2126 * p[0] + 0.7152 * p[1] + 0.0722 * p[2];
        if is_metal(i) {
            metallic.push(y)
        } else {
            dielectric.push(y)
        }
    }
    if let Some((p1, p99)) = spread(dielectric) {
        if p1 < 0.013 || p99 > 0.871 {
            out.push((
                "albedo",
                format!("dielectric luminance p1 {p1:.3}, p99 {p99:.3}: want 0.013–0.871"),
            ));
        }
    }
    if let Some((p1, _)) = spread(metallic) {
        if p1 < 0.45 {
            out.push(("albedo", format!("metal luminance p1 {p1:.3}: want ≥ 0.45")));
        }
    }
    let width = |v: Vec<f32>| spread(v).map_or(0.0, |(a, b)| b - a);
    let w = width(px(rough, 0));
    if w < 0.1 {
        out.push(("roughness", format!("flat: p99 − p1 = {w:.3}, want ≥ 0.1")));
    }
    let blue = px(normal, 2).into_iter().fold(1.0, f32::min);
    let tilt = width(px(normal, 0)).max(width(px(normal, 1)));
    if blue <= 0.5 || tilt < 0.02 {
        out.push((
            "normal",
            format!("lowest blue {blue:.3} (want > 0.5), tilt spread {tilt:.3} (want ≥ 0.02)"),
        ));
    }
    let h = width(px(height, 0));
    if h < 0.5 {
        out.push(("height", format!("p99 − p1 = {h:.3}, want ≥ 0.5")));
    }
    out
}

/// `(p1, p99)` by nearest rank, or `None` for no values.
fn spread(mut v: Vec<f32>) -> Option<(f32, f32)> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f32::total_cmp);
    let at = |q: f32| v[((v.len() - 1) as f32 * q).round() as usize];
    Some((at(0.01), at(0.99)))
}

/// Why `img` does not tile, if it doesn't: on either axis the mean jump across the
/// wrap is well above every mean jump between two neighbouring interior lines.
pub fn seam(img: &Image) -> Option<String> {
    let (w, h) = (img.width() as i64, img.height() as i64);
    for across in [true, false] {
        let (n, m) = if across { (w, h) } else { (h, w) };
        let at = |i: i64, j: i64| {
            if across {
                img.get_wrapped(i, j)
            } else {
                img.get_wrapped(j, i)
            }
        };
        let line = |i: i64| -> f32 {
            let jump = |j| {
                (0..4)
                    .map(|c| (at(i, j)[c] - at(i + 1, j)[c]).abs())
                    .fold(0.0, f32::max)
            };
            (0..m).map(jump).sum::<f32>() / m as f32
        };
        let interior = (0..n - 1).map(line).fold(0.0, f32::max);
        let wrap = line(n - 1);
        if wrap > interior * 1.25 + 1e-3 {
            let axis = if across { "across" } else { "down" };
            return Some(format!(
                "seam {axis}: mean jump {wrap:.4} at the wrap, {interior:.4} at most inside"
            ));
        }
    }
    None
}

#[test]
fn the_seam_check_flags_a_ramp_and_passes_a_tiling_noise() {
    let ramp = Image::fill_uv(64, |u, _| [u, u, u, 1.0]);
    assert!(seam(&ramp).is_some(), "a 0→1 ramp has a seam");
    let wave = Image::fill_uv(64, |u, v| {
        let s = (std::f32::consts::TAU * (3.0 * u + 2.0 * v)).sin() * 0.5 + 0.5;
        [s, s, s, 1.0]
    });
    assert!(seam(&wave).is_none(), "a whole-period wave tiles");
}

#[test]
fn the_albedo_rule_flags_coal_and_passes_concrete() {
    let flat = |v: f32| Image::filled(16, [v, v, v, 1.0]);
    let set = |b: f32| {
        vec![
            flat(b),
            Image::filled(16, [0.5, 0.5, 1.0, 1.0]),
            flat(0.5),
            flat(0.5),
            flat(1.0),
        ]
    };
    let rules = |b| {
        maps_fail(&set(b), None)
            .into_iter()
            .map(|(r, _)| r)
            .collect::<Vec<_>>()
    };
    assert!(
        rules(0.005).contains(&"albedo"),
        "a near-black dielectric fails"
    );
    assert!(!rules(0.2).contains(&"albedo"), "a mid-grey one passes");
}
