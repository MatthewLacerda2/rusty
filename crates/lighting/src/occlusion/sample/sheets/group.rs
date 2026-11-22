use glam::Vec3;

use super::{Member, Sheet, CLEAR};

const NOISE: f32 = 8.0 * f32::EPSILON;

type Keyed = (([i64; 3], i64, u8), u32);

pub(in super::super) fn sheets(faces: &[Member]) -> (Vec<Sheet>, Vec<u32>) {
    // Each face keyed by its plane and side, sorted: a plane's faces then run
    // together, in face order.
    let key = |f: &Member| {
        let n = (f.normal * 1e6).round().as_i64vec3().to_array();
        (
            n,
            (f.normal.dot(f.corners[0]) / CLEAR).round() as i64,
            f.face as u8,
        )
    };
    let planar = faces
        .iter()
        .enumerate()
        .filter(|(_, f)| f.normal != Vec3::ZERO);
    let mut keyed: Vec<_> = planar.map(|(i, f)| (key(f), i as u32)).collect();
    keyed.sort_unstable();
    let mut of = vec![u32::MAX; faces.len()];
    let mut sheets = Vec::new();
    let runs = keyed.chunk_by(|a, b| a.0 == b.0).filter(|r| r.len() > 1);
    for run in runs.flat_map(|run| depths(faces, run)) {
        let sheet = Sheet::of(run.iter().map(|&i| &faces[i as usize]));
        let Some(sheet) = sheet.filter(|s| !s.sparse()) else {
            continue;
        };
        for &i in &run {
            of[i as usize] = sheets.len() as u32;
        }
        sheets.push(sheet);
    }
    (sheets, of)
}

pub(super) fn tolerance<'a>(points: impl Iterator<Item = &'a Vec3>) -> f32 {
    let scale = points.fold(1.0f32, |m, p| m.max(p.abs().max_element()));
    NOISE * scale
}

fn depths(faces: &[Member], run: &[Keyed]) -> Vec<Vec<u32>> {
    let normal = faces[run[0].1 as usize].normal;
    let all = run.iter().flat_map(|&(_, i)| faces[i as usize].corners);
    let tol = tolerance(all);
    let mut slabs: Vec<(f32, f32, u32)> = (run.iter())
        .map(|&(_, i)| {
            let d = faces[i as usize].corners.iter().map(|&p| normal.dot(p));
            let (lo, hi) = d.fold((f32::MAX, f32::MIN), |(l, h), d| (l.min(d), h.max(d)));
            (lo, hi, i)
        })
        .filter(|&(lo, hi, _)| hi - lo <= tol)
        .collect();
    slabs.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.2.cmp(&b.2)));
    let mut out: Vec<Vec<u32>> = Vec::new();
    let mut from = f32::NEG_INFINITY;
    for (lo, hi, i) in slabs {
        match out.last_mut().filter(|_| hi - from <= tol) {
            Some(group) => group.push(i),
            None => {
                from = lo;
                out.push(vec![i]);
            }
        }
    }
    out.retain(|g| g.len() > 1);
    out.iter_mut().for_each(|g| g.sort_unstable());
    out
}
