//! The sweeps' gather through the caster BVH follows every edit (#1874): a static
//! object moved far, one made dynamic, one added after the build and one that
//! comes to cast per frame are gathered where they are now, not where the BVH last saw them; a stray reaches every
//! volume until a rebuild, which past the strays' limit comes on its own.
//! Skips with no adapter.

use glam::camera::rh::proj::directx;
use glam::camera::rh::view::look_to_mat4;
use glam::{Mat4, Vec3};

use super::super::fixture::{yard, RES};
use crate::render::Renderer;
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::Scene;

/// A short point-light face at `x` along the yard's row, looking down.
fn at(x: f32) -> Mat4 {
    let face = directx::perspective(std::f32::consts::FRAC_PI_2, 1.0, 0.05, 2.0);
    face * look_to_mat4(Vec3::new(x, 1.0, -4.0), -Vec3::Y, Vec3::Z)
}

/// The ids a static bake (`baked`) or a per-frame sweep gathers for the face at
/// `x`, after an extract of `scene`.
fn near(r: &mut Renderer, scene: &Scene, x: f32, baked: bool) -> Vec<u32> {
    r.extract_scene(scene);
    let kept = r.retained.scene(scene.id()).expect("extracted");
    let near = kept.casters_near(&[at(x)], baked);
    near.iter().map(|&(id, _)| id).collect()
}

#[test]
fn gpu_the_gather_follows_moved_flipped_and_new_casters() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut scene = yard(6);
    let ids = scene.world.ids_with_mesh();
    ids.iter()
        .for_each(|&id| _ = scene.world.set_static(id, true));
    assert!(
        near(&mut r, &scene, 0.0, true).contains(&ids[2]),
        "Prop_2 at x = 0"
    );
    assert!(
        near(&mut r, &scene, 40.0, true).is_empty(),
        "nothing out there yet"
    );
    assert!(
        near(&mut r, &scene, 0.0, false).is_empty(),
        "nothing casts per frame"
    );

    scene.world.transform_mut(ids[2]).unwrap().position.x = 40.0;
    assert_eq!(
        near(&mut r, &scene, 40.0, true),
        [ids[2]],
        "moved: gathered there"
    );
    scene.world.set_static(ids[3], false);
    scene.world.transform_mut(ids[3]).unwrap().position.x = 40.5;
    assert_eq!(near(&mut r, &scene, 40.0, true), [ids[2]], "made dynamic");
    assert_eq!(
        near(&mut r, &scene, 0.0, false),
        [ids[3]],
        "dynamic: every sweep"
    );
    let new = create_entity(&mut scene, "Late", Some(Primitive::Box));
    scene.world.transform_mut(new).unwrap().position = Vec3::new(39.5, 0.0, -4.0);
    scene.world.set_static(new, true);
    assert_eq!(near(&mut r, &scene, 40.0, true), [ids[2], new], "added");
    // A small-culled static casts per frame wherever it is (#1714).
    scene.world.mesh_mut(ids[4]).unwrap().max_draw_distance = 30.0;
    assert_eq!(
        near(&mut r, &scene, 90.0, false),
        [ids[3], ids[4]],
        "per frame"
    );

    // A stray is handed to every volume until a rebuild.
    scene.world.transform_mut(ids[0]).unwrap().position.x = 60.0;
    assert_eq!(
        near(&mut r, &scene, 90.0, true),
        [ids[0]],
        "a stray, anywhere"
    );
    assert_eq!(near(&mut r, &scene, 90.0, false), [ids[0], ids[3], ids[4]]);

    // Past the strays' limit the BVH is rebuilt where they are now.
    let mut big = yard(90);
    let all = big.world.ids_with_mesh();
    all.iter()
        .for_each(|&id| _ = big.world.set_static(id, true));
    near(&mut r, &big, 0.0, true);
    for (i, &id) in all.iter().enumerate() {
        let mut t = big.world.transform_mut(id).unwrap();
        (t.position.y, t.position.z) = (0.01, -4.0 - 100.0 * (i % 2) as f32);
    }
    let row = near(&mut r, &big, 0.0, true);
    let kept = r.retained.scene(big.id()).expect("extracted");
    assert_eq!(kept.caster_index().strays(), 0, "rebuilt past the limit");
    assert!(row
        .iter()
        .all(|id| all.iter().position(|a| a == id).unwrap() % 2 == 0));
    assert!(!row.is_empty(), "the row's casters near x = 0");
}
