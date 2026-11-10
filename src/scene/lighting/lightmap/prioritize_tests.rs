//! Prioritize View converges what the camera sees first (#1970's review): at a
//! shown step the view's texels hold more paths than the rest, and the camera's
//! own test reads orthographic views and lets a `ShadowsOnly` mesh hide nothing.

use glam::Vec3;

use super::bake::progressive::progressive_on;
use super::bake::progressive::schedule::{steps, Step};
use super::progressive_tests::camera;
use super::tests::{floor, quad, settings, wall};
use super::*;
use crate::components::ShadowCasting;

#[test]
fn the_view_converges_before_the_rest() {
    // The wall seen, the floor not (as `view_tests`), lit by the sky alone: what a
    // step shows of a texel is then exactly its paths so far, as the last resolves.
    let view = camera(Vec3::new(-3.0, 2.0, 0.0), Vec3::new(2.0, 2.0, 0.0), 0.3);
    let scene = BakeScene {
        meshes: vec![floor(), wall()],
        sky: Vec3::new(0.3, 0.4, 0.5).into(),
        ..Default::default()
    };
    let s = BakeSettings {
        samples: 4,
        samples_per_pass: 1,
        ..settings()
    };
    let mut shown: Vec<(Pass, Vec<Lightmap>)> = Vec::new();
    let mut heard = |pass: Pass, maps: Vec<Lightmap>| shown.push((pass, maps));
    let progress = BakeProgress::default();
    let last = progressive_on(&scene, &s, &progress, (Some(view), 2), &mut heard);
    let last = last.expect("finished");
    let one_shot = bake::bake_on(&scene, &s, &BakeProgress::default(), 2).expect("finished");
    assert_eq!(last, one_shot, "the same bytes");
    // 1 path for all, 3 more for the wall, then 3 more for the floor.
    assert_eq!(shown.len(), 6);
    let (pass, maps) = &shown[3];
    assert_eq!((pass.done, pass.passes), (4, 7));
    assert_eq!((pass.samples, pass.view, pass.target), (1, 4, 4));
    assert_eq!(maps[1].texels, last[1].texels, "the wall is finished");
    assert_ne!(maps[0].texels, last[0].texels, "the floor is not");
}

#[test]
fn a_view_of_all_or_none_of_the_scene_runs_every_pass_over_every_texel() {
    let s = |seen, rest| Step { seen, rest };
    assert_eq!(steps(10, 4, false), [s(4, 4), s(4, 4), s(2, 2)]);
    let split = steps(10, 4, true);
    assert_eq!(split, [s(4, 4), s(4, 0), s(2, 0), s(0, 4), s(0, 2)]);
    let sum = |seen| split.iter().map(|x| x.of(seen)).sum::<u32>();
    assert_eq!((sum(true), sum(false)), (10, 10));
    assert_eq!(
        steps(4, 64, true),
        [s(4, 4)],
        "one pass: nothing runs ahead"
    );
}

#[test]
fn an_orthographic_view_faces_along_its_direction_and_shadows_only_hides_nothing() {
    let blocker = |cast_shadows| BakeMesh {
        cast_shadows,
        ..quad(
            5,
            [
                Vec3::new(0.0, 0.0, -2.0),
                Vec3::new(0.0, 4.0, -2.0),
                Vec3::new(0.0, 4.0, 2.0),
                Vec3::new(0.0, 0.0, 2.0),
            ],
            Vec3::ONE,
        )
    };
    let rays = |cast| {
        let meshes = vec![floor(), wall(), blocker(cast)];
        let scene = BakeScene {
            meshes,
            ..Default::default()
        };
        rays::Rays::build(bake::triangles(&scene))
    };
    let front = Vec3::new(2.0, 2.0, 0.0);
    let perspective = camera(Vec3::new(-3.0, 2.0, 0.0), front, 1.0);
    assert!(!perspective.sees(&rays(ShadowCasting::On), front, Vec3::NEG_X));
    let hidden = rays(ShadowCasting::ShadowsOnly);
    assert!(
        perspective.sees(&hidden, front, Vec3::NEG_X),
        "drawn by no camera"
    );
    // Looking down +X from x = -3, 8 units wide, the eye far off to the side: an
    // orthographic view ignores it, and faces the wall square on.
    let proj = glam::camera::rh::proj::directx::orthographic(-4.0, 4.0, -4.0, 4.0, 0.05, 50.0);
    let look = glam::camera::rh::view::look_at_mat4(Vec3::new(-3.0, 2.0, 0.0), front, Vec3::Y);
    let ortho = BakeView {
        eye: Vec3::new(0.0, 40.0, 40.0),
        view_proj: proj * look,
    };
    let open = rays(ShadowCasting::ShadowsOnly);
    assert!(ortho.sees(&open, front, Vec3::NEG_X));
    assert!(!ortho.sees(&open, front, Vec3::Y), "edge-on to the view");
    assert!(!ortho.sees(&rays(ShadowCasting::On), front, Vec3::NEG_X));
}
