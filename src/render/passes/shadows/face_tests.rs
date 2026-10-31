//! A material's Render Face (#1615) in the shadow casters, as Unity's ShadowCaster
//! pass culls by `_Cull`: a one-sided card casts from the faces its material draws.
//! `Front` casts only while its front faces the light, `Back` only while its back
//! does, and `Both` either way, as a `TwoSided` caster (#1571). Skips with no adapter.

use super::casting_tests::card;
use super::mirror_tests::sun_yard;
use crate::components::{RenderFace, ShadowCasting};
use crate::render::gpu::pipelines::surface::cut_depth_tests::ground_halves;
use crate::render::Renderer;
use crate::scene::authoring::material;

/// The ground's brightness under each half of a card drawing `face`, its front up
/// toward the sun or, with `down`, away from it; none for `None`.
fn shade(r: &mut Renderer, face: Option<RenderFace>, down: bool) -> [f32; 2] {
    let mut scene = sun_yard();
    if let Some(face) = face {
        let id = card(&mut scene, ShadowCasting::On, down, false);
        let key = material::ensure_material_key(&mut scene, id).unwrap();
        scene.materials.get_mut(&key).unwrap().render_face = face;
    }
    ground_halves(r, &scene)
}

#[test]
fn gpu_a_card_casts_from_the_faces_its_material_draws() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(32, 32) else {
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
