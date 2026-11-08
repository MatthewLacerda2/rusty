//! What the lightmap bake costs per texel (#1883): traces a spread of a scene's
//! texels at the given settings, for `dev::bake`'s hand-run measurement. A whole
//! bake of a Dust2-sized level runs for hours, so the bench samples: about
//! `count` texels taken evenly across the receiving meshes.

use std::thread;

use super::super::atlas::max_map_edge;
use super::super::bake::{triangles, BakeSettings};
use super::super::input::BakeScene;
use super::super::raster::{lightmap_size, rasterize, TexelPoint};
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::Tracer;

/// Trace about `count` texels of `scene` spread evenly over its receiving meshes,
/// on `threads` workers, exactly as the bake traces each one; returns how many
/// were traced.
pub fn trace_texels(
    scene: &BakeScene,
    settings: &BakeSettings,
    count: usize,
    threads: usize,
) -> usize {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let max = max_map_edge(settings.page());
    let receivers: Vec<usize> = (0..scene.meshes.len())
        .filter(|&i| scene.meshes[i].receives_lightmap())
        .collect();
    let meshes = receivers.len().clamp(1, 64);
    let per_mesh = count.div_ceil(meshes);
    let mut points: Vec<(u64, TexelPoint)> = Vec::new();
    for k in 0..meshes.min(receivers.len()) {
        let index = receivers[k * receivers.len() / meshes];
        let mesh = &scene.meshes[index];
        let density = settings.texels_per_unit * mesh.scale_in_lightmap;
        let Some(size) = lightmap_size(mesh, density, max) else {
            continue;
        };
        let all = rasterize(mesh, size);
        let step = (all.len() / per_mesh).max(1);
        let picked = all.into_iter().step_by(step).take(per_mesh);
        points.extend(picked.map(|p| (index as u64, p)));
    }
    let chunk = points.len().div_ceil(threads.max(1)).max(1);
    thread::scope(|s| {
        for part in points.chunks(chunk) {
            let tracer = &tracer;
            s.spawn(move || {
                for (mesh, p) in part {
                    let mut rng = Rng::new(settings.seed, *mesh, p.index as u64);
                    let (n, b) = (settings.samples, settings.bounces);
                    std::hint::black_box(tracer.texel(p.position, p.normal, n, b, &mut rng));
                }
            });
        }
    });
    points.len()
}
