//! Each material run of a mesh clips its own shadow, per entity (#1285). Two slabs
//! share one two-run mesh: one draws both runs opaque, the other's run 1 (its +X
//! half) is a Cutout over a clear texture. The second slab's +X half casts nothing;
//! its -X half still does. Before runs keyed their draws by start and end, the
//! second slab's run 0 joined the first slab's whole-mesh draw and cast both halves.
//! Skips with no adapter.

use glam::Vec3;

use crate::components::mesh::Vertex;
use crate::components::{MaterialAsset, MaterialComponent, MeshComponent, RenderMode};
use crate::render::gpu::pipelines::surface::cut_depth_tests::{ground_halves, slab_yard};

/// A unit plane in XZ split at x = 0 into two runs (-X, then +X), each face wound
/// both ways so it casts from either side.
fn halves() -> MeshComponent {
    let (mut vertices, mut indices, mut ranges) = (Vec::new(), Vec::new(), Vec::new());
    for (x0, x1) in [(-0.5, 0.0), (0.0, 0.5)] {
        let base = vertices.len() as u32;
        for (x, z) in [(x0, -0.5), (x1, -0.5), (x1, 0.5), (x0, 0.5)] {
            vertices.push(Vertex::new(
                Vec3::new(x, 0.0, z),
                Vec3::Y,
                [x + 0.5, z + 0.5],
            ));
        }
        let start = indices.len() as u32;
        indices.extend([0, 2, 1, 0, 3, 2, 0, 1, 2, 0, 2, 3].map(|i| base + i));
        ranges.push(start..indices.len() as u32);
    }
    MeshComponent {
        ranges,
        ..MeshComponent::new("RunHalves", vertices, indices)
    }
}

#[test]
fn gpu_one_entitys_cutout_run_does_not_join_anothers_whole_mesh_shadow() {
    let clear = image::RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 0]));
    let path = crate::test_temp::dir().join("clear.png");
    clear.save(&path).expect("texture written");
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let leaf = MaterialAsset {
        render_mode: RenderMode::Cutout,
        base_color_map: Some(path.to_string_lossy().into_owned().into()),
        ..MaterialAsset::default()
    };
    let both = |a: &str, b: &str| MaterialComponent {
        materials: vec![a.into(), b.into()],
    };
    // The yard's slab becomes the twin: both runs opaque, moved off to the side. It
    // comes first in scene order, so a merge would hand its whole-mesh run on.
    let (mut scene, twin) = slab_yard(None);
    scene
        .materials
        .insert("solid".into(), MaterialAsset::default());
    scene.materials.insert("leaf".into(), leaf);
    scene.world.set_mesh(twin, Some(halves()));
    scene.world.set_material(twin, Some(both("solid", "solid")));
    let mut t = scene.world.transform_mut(twin).unwrap();
    let place = (t.position, t.scale);
    t.position.z += 9.0;
    drop(t);
    // The slab under test, where the twin was: its +X run is the clear cutout.
    let slab = scene.add_entity("Slab".into());
    scene.world.set_mesh(slab, Some(halves()));
    scene.world.set_material(slab, Some(both("solid", "leaf")));
    let mut t = scene.world.transform_mut(slab).unwrap();
    (t.position, t.scale) = place;
    drop(t);
    let [kept, clipped] = ground_halves(&mut r, &scene);
    eprintln!("[run shadow] -X {kept:.1}, +X {clipped:.1}");
    assert!(clipped > kept + 20.0, "the cutout run casts nothing");
}
