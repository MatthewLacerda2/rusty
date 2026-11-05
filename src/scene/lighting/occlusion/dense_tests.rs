//! What the bake makes of meshes that are not boxes (#1714): a dense prop gets
//! about the targets a box of its size does and is culled behind a wall, and a
//! mesh with a vertex or a matrix that is not finite bakes without a panic and is
//! never culled.

use glam::Vec3;

use super::gather::Fingerprints;
use super::targets::target_points;
use super::test_level::{block, corridors};
use super::{bake_occlusion, OcclusionData};
use crate::components::mesh::{primitives, MeshRevision};
use crate::scene::Scene;

/// The north room's corner, and where the far south room keeps a prop.
const CORNER: Vec3 = Vec3::new(1.0, 1.6, 8.0);
const SOUTH: Vec3 = Vec3::new(15.0, 0.75, -5.0);

/// A static 1.5 m sphere of about 2000 triangles at `at`.
fn dense(scene: &mut Scene, at: Vec3) -> u32 {
    let id = block(
        scene,
        "Statue",
        at - Vec3::splat(0.75),
        at + Vec3::splat(0.75),
    );
    let (vertices, indices) = primitives::generate_sphere(0.5, 32, 32);
    let mut mesh = scene.world.mesh_mut(id).expect("a box");
    (mesh.vertices, mesh.indices) = (vertices, indices);
    mesh.revision = MeshRevision::next();
    id
}

/// Entity `id`'s target count, `None` when it is never culled.
fn targets(scene: &Scene, id: u32) -> Option<usize> {
    let object = Fingerprints::of_scene(scene).object(scene, id)?;
    target_points(&object.drawn).map(|t| t.len())
}

fn seen_from(data: &OcclusionData, eye: Vec3, id: u32) -> bool {
    let row = data.row_at(eye).expect("in the grid");
    data.sees(row, data.object_of(id).expect("baked"))
}

#[test]
fn a_dense_prop_gets_the_targets_a_box_of_its_size_does_and_is_culled() {
    let mut scene = corridors();
    let statue = dense(&mut scene, SOUTH);
    let size = Vec3::splat(0.75);
    let crate_ = block(
        &mut scene,
        "Crate",
        SOUTH - Vec3::X * 3.0 - size,
        SOUTH - Vec3::X * 3.0 + size,
    );
    let triangles = scene.world.mesh(statue).unwrap().indices.len() / 3;
    assert!(triangles >= 1900, "{triangles} triangles");
    let (dense, boxed) = (targets(&scene, statue), targets(&scene, crate_));
    let (dense, boxed) = (dense.expect("sampled"), boxed.expect("sampled"));
    // A sphere has less surface than its box, and its points fold into voxels.
    assert!(
        dense <= boxed * 2,
        "{dense} targets against a box's {boxed}"
    );
    let data = bake_occlusion(&scene, 4).0.expect("occluders");
    assert!(!seen_from(&data, CORNER, statue), "behind two walls");
    assert!(
        seen_from(&data, SOUTH + Vec3::Z * 2.0, statue),
        "in its own room"
    );
}

#[test]
fn a_mesh_that_is_not_finite_bakes_and_is_never_culled() {
    let mut scene = corridors();
    let vertex = dense(&mut scene, SOUTH);
    let mut mesh = scene.world.mesh_mut(vertex).unwrap();
    mesh.vertices[5].position[1] = f32::NAN;
    mesh.revision = MeshRevision::next();
    drop(mesh);
    let matrix = block(&mut scene, "Nan", SOUTH, SOUTH + Vec3::ONE);
    scene.world.transform_mut(matrix).unwrap().scale = Vec3::new(1.0, f32::NAN, 1.0);
    let far = block(&mut scene, "Far", SOUTH, SOUTH + Vec3::ONE);
    scene.world.transform_mut(far).unwrap().position = Vec3::splat(f32::INFINITY);
    let (data, report) = bake_occlusion(&scene, 2);
    let data = data.expect("the rest bakes");
    assert!(report.never_culled >= 3, "{report:?}");
    for id in [vertex, matrix, far] {
        let object = data.object_of(id).expect("baked");
        assert!((0..data.row_count() as u32).all(|row| data.sees(row, object)));
    }
    // They lay no cells: the grid is the corridor level's, of 2 m.
    assert_eq!(data.grid.cell, super::CELL_SIZE);
}
