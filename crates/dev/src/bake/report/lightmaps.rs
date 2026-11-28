use std::collections::BTreeMap;
use std::time::Duration;

use serde_json::{json, Value};

use super::{entity_name, point, secs, Kind, Report, Stages};
use crate::bake::lightmap::EncodedPages;
use crate::bake::progress::MeshTime;
use rusty_components::ReceiveGi;
use rusty_lighting::lightmap::{BakeLight, BakeMesh, BakeScene, LightShape};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

#[derive(Debug, Default)]
pub struct Measured {
    pub times: Vec<MeshTime>,
    pub denoised: Option<(u64, Duration)>,
    pub peak_memory: Option<u64>,
}

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
    let probe_lit: Vec<String> = (input.meshes.iter())
        .filter(|m| m.receive_gi == ReceiveGi::LightProbes)
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
        "receiveLightProbes": probe_lit,
        "denoise": denoise,
        "peakResidentMB": measured.peak_memory.map(|b| b / (1 << 20)),
        "lights": lights.collect::<Vec<_>>(),
        "invalidTexels": Value::Null,
    }))
}

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
    // Slowest first: `None` sorts below any time, so untimed meshes come last.
    meshes.sort_by_key(|m| std::cmp::Reverse(m.0));
    (meshes.into_iter().map(|m| m.1).collect(), texels_lit)
}

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

/// UV charts: a UV seam splits a vertex, so triangles joined through shared vertices are one.
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

fn fingerprint(input: &BakeScene, settings: &BakeSettings) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    for m in &input.meshes {
        hash.write(&m.entity.to_le_bytes());
        hash.write(bytemuck::cast_slice(&m.positions));
        hash.write(bytemuck::cast_slice(&m.normals));
        hash.write(bytemuck::cast_slice(&m.lightmap_uvs));
        hash.write(bytemuck::cast_slice(&m.indices));
        let rest = (&m.runs, m.cast_shadows, m.scale_in_lightmap);
        hash.write(format!("{rest:?}").as_bytes());
        if m.receive_gi != ReceiveGi::default() {
            hash.write(m.receive_gi.name().as_bytes());
        }
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
