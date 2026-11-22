//! An edit in place of one instance of a shared model is drawn on that instance
//! alone: `MeshComponent::vertices_mut` gives it geometry of its own, and the
//! renderer, which keeps one GPU slot per mesh source, keys the edited one apart, so
//! its sibling still draws the import's shape (Unity's `MeshFilter.mesh` over
//! `sharedMesh`).
//!
//! Two quads of one `.obj` side by side, shot before and after the left one's
//! vertices are lifted: the right half's pixels are byte-identical, the left's moved.
//! The same capture host takes both shots, so its renderer keeps the slots between
//! them, as a running game's does.

use crate::temp::{dir, file as tmp};
use glam::Vec3;
use rusty::dev::capture::CaptureHost;
use rusty::dev::screenshot::capture_into;
use rusty::scene::{asset_mesh_component, Camera, Scene};

/// A one-metre quad facing +Z, one object named `Model`.
const QUAD: &str = "o Model\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nf 1 2 3\nf 1 3 4\n";
const SIZE: u32 = 64;

/// The `(left, right)` halves of the capture at `path`, as RGB bytes.
fn halves(path: &std::path::Path) -> (Vec<u8>, Vec<u8>) {
    let img = image::open(path).expect("open the capture").to_rgb8();
    let (mut left, mut right) = (Vec::new(), Vec::new());
    for (x, _, p) in img.enumerate_pixels() {
        let half = if x < SIZE / 2 { &mut left } else { &mut right };
        half.extend_from_slice(&p.0);
    }
    (left, right)
}

#[test]
fn an_edited_instance_draws_its_edit_and_its_sibling_keeps_the_import() {
    let path = dir().join("shared_quad.obj");
    std::fs::write(&path, QUAD).unwrap();
    let reference = format!("{}::Model", path.to_string_lossy());
    let mut scene = Scene::new();
    let ids = [-1.75_f32, 0.75].map(|x| {
        let id = scene.add_entity(format!("Quad {x}"));
        scene
            .world
            .set_mesh(id, Some(asset_mesh_component(&reference)));
        scene.world.transform_mut(id).unwrap().position = Vec3::new(x, -0.5, 0.0);
        id
    });
    // Facing -Z from three metres out, both quads in view.
    let camera = Camera::new(Vec3::new(0.0, 0.0, 3.0), -90.0, 0.0);
    let mut host = CaptureHost::new();
    let (before, after) = (
        tmp("rusty_shared_edit_before.png"),
        tmp("rusty_shared_edit_after.png"),
    );
    if !capture_into(&mut host, &scene, &camera, &before, SIZE, SIZE).expect("capture") {
        eprintln!("[shared-mesh-edit] no GPU/software adapter — skipping visual assertion");
        return;
    }
    let mut edited = scene.world.mesh_mut(ids[0]).unwrap();
    edited
        .vertices_mut()
        .iter_mut()
        .for_each(|v| v.position[1] += 0.6);
    drop(edited);
    capture_into(&mut host, &scene, &camera, &after, SIZE, SIZE).expect("capture");

    let ((left0, right0), (left1, right1)) = (halves(&before), halves(&after));
    assert!(
        right0 == right1,
        "the sibling draws the import's quad still"
    );
    assert!(left0 != left1, "the edited instance draws its lifted quad");
}
