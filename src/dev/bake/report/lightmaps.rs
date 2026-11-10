//! The lightmap bake's report (#1980): per mesh its lightmap edge and texels, UV
//! charts, worker seconds and texels per second, the lights in reach, and the
//! texel density it asked for beside the one it got; per page how full it is;
//! the meshes at Scale In Lightmap 0; what OIDN took; per light the texels in its
//! reach. A light is in reach of a mesh when the mesh's bounds meet the sphere of
//! its range (a directional light reaches every mesh), so a spot light's cone is
//! not read. What only the tracer knows (the backface texels it filled) is not in
//! it.

use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{json, Value};

use super::{entity_name, point, secs, Kind, Report, Stages};
use crate::dev::bake::lightmap::EncodedPages;
use crate::dev::bake::progress::MeshTime;
use crate::scene::lighting::lightmap::{BakeLight, BakeMesh, BakeScene, BakeSettings, LightShape};
use crate::scene::Scene;

/// What a lightmap bake measured beside its pages.
#[derive(Debug, Default)]
pub struct Measured {
    /// Each mesh's worker time, over every thread.
    pub times: Vec<MeshTime>,
    /// The lightmaps OIDN cleaned and its worker time.
    pub denoised: Option<(u64, Duration)>,
}

/// The report of a bake of `scene` that read `input` with `settings`, ran
/// `stages`, made `pages` and wrote `output_bytes`.
pub fn report(
    scene: &Scene,
    (input, settings): (&BakeScene, &BakeSettings),
    (pages, measured): (&EncodedPages, &Measured),
    stages: &Stages,
    output_bytes: u64,
) -> Report {
    let fingerprint = fingerprint(input, settings);
    let (meshes, texels_lit) = mesh_rows(scene, (input, settings), (pages, measured));
    let zero: Vec<String> = (input.meshes.iter())
        .filter(|m| m.scale_in_lightmap <= 0.0)
        .map(|m| entity_name(scene, m.entity))
        .collect();
    let lights = input
        .lights
        .iter()
        .zip(&texels_lit)
        .enumerate()
        .map(|(i, (l, &t))| light(i, l, t));
    let denoise = measured
        .denoised
        .map(|(charts, busy)| json!({ "charts": charts, "seconds": secs(busy) }));
    Report::new(Kind::Lightmaps, stages, settings, fingerprint, output_bytes).with(json!({
        "maps": pages.maps,
        "pageSize": pages.page_size,
        "pages": occupancy(pages),
        "meshes": meshes,
        "zeroScaleInLightmap": zero,
        "denoise": denoise,
        "lights": lights.collect::<Vec<_>>(),
        "invalidTexels": Value::Null,
    }))
}

/// One row per lightmapped mesh, slowest first, and per light the texels in its
/// reach.
fn mesh_rows(
    scene: &Scene,
    (input, settings): (&BakeScene, &BakeSettings),
    (pages, measured): (&EncodedPages, &Measured),
) -> (Vec<Value>, Vec<u64>) {
    let page = pages.page_size as f32;
    let reach: Vec<Vec<usize>> = input
        .meshes
        .iter()
        .map(|m| lights_in_reach(m, &input.lights))
        .collect();
    let mut texels_lit = vec![0u64; input.lights.len()];
    let mut meshes = Vec::new();
    // By entity, so a Dust2-sized bake's thousands of meshes look each up once.
    let entries: BTreeMap<u32, _> = pages.entries.iter().map(|e| (e.entity, e)).collect();
    let busy_of: BTreeMap<u32, _> = measured.times.iter().map(|t| (t.entity, t.busy)).collect();
    for (mesh, lights) in input.meshes.iter().zip(&reach) {
        let Some(entry) = entries.get(&mesh.entity) else {
            continue;
        };
        let [sx, sy, ..] = entry.scale_offset;
        let (w, h) = ((sx * page).round() as u32, (sy * page).round() as u32);
        let texels = w as u64 * h as u64;
        lights.iter().for_each(|&l| texels_lit[l] += texels);
        let busy = busy_of.get(&mesh.entity).copied();
        let rate = busy
            .filter(|b| !b.is_zero())
            .map(|b| (texels as f64 / b.as_secs_f64()).round());
        meshes.push((
            busy,
            json!({
                "entity": mesh.entity,
                "name": entity_name(scene, mesh.entity),
                "page": entry.page,
                "size": [w, h],
                "texels": texels,
                "charts": charts(mesh),
                "seconds": busy.map(secs),
                "texelsPerSecond": rate,
                "lightsInReach": lights.len(),
                "requestedDensity": settings.texels_per_unit * mesh.scale_in_lightmap,
                "actualDensity": mesh.texel_density(w.max(h)),
            }),
        ));
    }
    // Slowest first; untimed ones after, in entity order.
    meshes.sort_by_key(|m| std::cmp::Reverse(m.0));
    (meshes.into_iter().map(|m| m.1).collect(), texels_lit)
}

/// Per page: its maps and the share of its texels they cover.
fn occupancy(pages: &EncodedPages) -> Value {
    let mut per = vec![(0usize, 0.0f64); pages.colour.len()];
    for e in &pages.entries {
        if let Some(p) = per.get_mut(e.page as usize) {
            *p = (
                p.0 + 1,
                p.1 + e.scale_offset[0] as f64 * e.scale_offset[1] as f64,
            );
        }
    }
    let page = |(i, (maps, full)): (usize, &(usize, f64))| json!({ "page": i, "maps": maps, "occupancy": (full * 1000.0).round() / 1000.0 });
    Value::Array(per.iter().enumerate().map(page).collect())
}

/// One light: its kind, where it is, its range and the texels in its reach.
fn light(index: usize, l: &BakeLight, texels: u64) -> Value {
    let (kind, at, range) = match l.shape {
        LightShape::Directional { .. } => ("directional", None, None),
        LightShape::Point { position, range } => ("point", Some(position), Some(range)),
        LightShape::Spot {
            position, range, ..
        } => ("spot", Some(position), Some(range)),
        LightShape::Area(a) => ("area", Some(a.center), Some(a.range)),
    };
    json!({
        "index": index,
        "kind": kind,
        "position": at.map(point),
        "range": range,
        "bakesDirect": l.bakes_direct,
        "texelsInReach": texels,
    })
}

/// The lights whose range meets `mesh`'s bounds, by index.
fn lights_in_reach(mesh: &BakeMesh, lights: &[BakeLight]) -> Vec<usize> {
    let at = mesh.positions.iter().copied();
    let (Some(min), Some(max)) = (
        at.clone().reduce(glam::Vec3::min),
        at.reduce(glam::Vec3::max),
    ) else {
        return Vec::new();
    };
    let near = |c: glam::Vec3, r: f32| c.clamp(min, max).distance(c) <= r;
    let reaches = |l: &BakeLight| match l.shape {
        LightShape::Directional { .. } => true,
        LightShape::Point { position, range }
        | LightShape::Spot {
            position, range, ..
        } => near(position, range),
        LightShape::Area(a) => near(a.center, a.range),
    };
    (lights.iter().enumerate())
        .filter(|(_, l)| reaches(l))
        .map(|(i, _)| i)
        .collect()
}

/// UV charts: the mesh's triangles joined through shared vertices (a UV seam
/// splits a vertex, so each chart is its own run of them).
pub(super) fn charts(mesh: &BakeMesh) -> usize {
    let mut parent: Vec<u32> = (0..mesh.positions.len() as u32).collect();
    fn root(parent: &mut [u32], mut i: u32) -> u32 {
        while parent[i as usize] != i {
            parent[i as usize] = parent[parent[i as usize] as usize];
            i = parent[i as usize];
        }
        i
    }
    let mut used = vec![false; parent.len()];
    for t in mesh.indices.chunks_exact(3) {
        if t.iter().any(|&i| i as usize >= parent.len()) {
            continue;
        }
        let a = root(&mut parent, t[0]);
        for &v in &t[1..] {
            let b = root(&mut parent, v);
            parent[b as usize] = a;
        }
        t.iter().for_each(|&i| used[i as usize] = true);
    }
    (0..parent.len() as u32)
        .filter(|&i| used[i as usize] && root(&mut parent, i) == i)
        .count()
}

/// What the bake reads: every mesh's geometry, runs and settings, the lights,
/// the sky, the diffuse model, the shadowmask channels, and the settings.
fn fingerprint(input: &BakeScene, settings: &BakeSettings) -> u64 {
    let mut hash = crate::core::hash::Content::default();
    for m in &input.meshes {
        hash.write(&m.entity.to_le_bytes());
        hash.write(bytemuck::cast_slice(&m.positions));
        hash.write(bytemuck::cast_slice(&m.normals));
        hash.write(bytemuck::cast_slice(&m.lightmap_uvs));
        hash.write(bytemuck::cast_slice(&m.indices));
        let rest = (&m.runs, m.cast_shadows, m.scale_in_lightmap);
        hash.write(format!("{rest:?}").as_bytes());
    }
    let rest = (
        &input.lights,
        &input.sky,
        input.diffuse,
        &input.shadowmask,
        settings,
    );
    hash.write(format!("{rest:?}").as_bytes());
    hash.finish()
}
