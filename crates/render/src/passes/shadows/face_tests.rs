use super::casting_tests::card;
use super::mirror_tests::sun_yard;
use super::statics_key;
use crate::gpu::pipelines::surface::cut_depth_tests::ground_halves;
use crate::Renderer;
use rusty_components::{RenderFace, ShadowCasting};

fn shade(r: &mut Renderer, face: Option<RenderFace>, down: bool) -> [f32; 2] {
    let mut scene = sun_yard();
    if let Some(face) = face {
        let id = card(&mut scene, ShadowCasting::On, down, false);
        let key = scene.ensure_material_key(id).unwrap();
        scene.materials.get_mut(&key).unwrap().render_face = face;
    }
    ground_halves(r, &scene)
}

#[test]
fn gpu_a_card_casts_from_the_faces_its_material_draws() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let open = shade(&mut r, None, false);
    let casts = |lit: [f32; 2]| (0..2).all(|i| lit[i] + 20.0 < open[i]);
    let open_ = |lit: [f32; 2]| (0..2).all(|i| (lit[i] - open[i]).abs() < 5.0);
    let mut both_ways = |face| [false, true].map(|down| shade(&mut r, Some(face), down));
    let [front_up, front_down] = both_ways(RenderFace::Front);
    let [back_up, back_down] = both_ways(RenderFace::Back);
    let [both_up, both_down] = both_ways(RenderFace::Both);
    eprintln!(
        "[render face shadow] open {open:?}; up/down: Front {front_up:?} {front_down:?}, \
         Back {back_up:?} {back_down:?}, Both {both_up:?} {both_down:?}"
    );
    assert!(
        casts(front_up) && open_(front_down),
        "control: Front casts facing up"
    );
    assert!(
        open_(back_up) && casts(back_down),
        "Back casts from its back face"
    );
    assert!(casts(both_up) && casts(both_down), "Both casts either way");
}

#[test]
fn a_static_cards_render_face_moves_the_bake_key() {
    let mut scene = sun_yard();
    let id = card(&mut scene, ShadowCasting::On, false, false);
    scene.world.set_static(id, true);
    let key = scene.ensure_material_key(id).unwrap();
    let mut key_as = |face| {
        scene.materials.get_mut(&key).unwrap().render_face = face;
        statics_key(&scene)
    };
    let front = key_as(RenderFace::Front);
    let both = key_as(RenderFace::Both);
    let back = key_as(RenderFace::Back);
    assert_ne!(both, front, "Both casts from its back faces too");
    assert_ne!(back, front);
    assert_ne!(back, both);
    assert_eq!(
        key_as(RenderFace::Front),
        front,
        "back to Front, back to its key"
    );
}
