//! The lens effects (#1439) into `PostParams`: what each packs, its clamps, and
//! what a tonemap-only view drops.

use super::*;

/// #1439: each lens effect packs its knobs when on, and a zero intensity (which the
/// composite skips) when off or forbidden by the player.
#[test]
fn lens_effects_pack_when_on_and_zero_when_off_or_forbidden() {
    let mut on = vc();
    on.vignette.active = true;
    on.vignette.center = [0.25, 0.75];
    on.chromatic_aberration.active = true;
    on.film_grain.active = true;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::Low);
    assert_eq!(p.vignette, [0.3, 0.2, 0.25, 0.75]);
    assert_eq!(p.lens, [0.3, 0.3, 0.8, 0.0]);
    let id_m = Mat4::IDENTITY;
    let post = borrow_post_source(&scene, Some(id));
    for (option, slot) in [
        (GraphicsOption::Vignette, 0),
        (GraphicsOption::ChromaticAberration, 1),
        (GraphicsOption::FilmGrain, 2),
    ] {
        let (p, _) = build_post_params(&scene, post.source(), forbidding(option), id_m, [1.0; 3]);
        let intensity = [p.vignette[0], p.lens[0], p.lens[1]];
        assert_eq!(intensity[slot], 0.0, "{option:?} forbidden");
    }
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::High);
    assert_eq!(
        (p.vignette[0], p.lens[0], p.lens[1]),
        (0.0, 0.0, 0.0),
        "off by default"
    );
}

/// #1439: a loaded file bypasses the ops' clamps, so packing clamps every lens knob
/// (a zero smoothness would make the vignette's `pow(0, 0)` a NaN).
#[test]
fn lens_knobs_are_clamped_when_packed() {
    let mut on = vc();
    (on.vignette.active, on.vignette.intensity) = (true, 5.0);
    (on.vignette.smoothness, on.vignette.color) = (0.0, [2.0, -1.0, 0.5]);
    (
        on.chromatic_aberration.active,
        on.chromatic_aberration.intensity,
    ) = (true, -1.0);
    (on.film_grain.active, on.film_grain.intensity) = (true, 3.0);
    on.film_grain.response = -2.0;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let (p, _) = params(&scene, Some(id), QualityPreset::High);
    let min = crate::components::Vignette::MIN_SMOOTHNESS;
    assert_eq!(p.vignette, [1.0, min, 0.5, 0.5]);
    assert_eq!(p.vignette_color, [1.0, 0.0, 0.5, 0.0]);
    assert_eq!(p.lens, [0.0, 1.0, 0.0, 0.0]);
}

/// #1439: a tonemap-only view (a render texture with post-FX off) drops the lens
/// effects with the grade.
#[test]
fn tonemap_only_drops_the_lens_effects() {
    let mut on = vc();
    on.vignette.active = true;
    on.chromatic_aberration.active = true;
    on.film_grain.active = true;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let (mut p, _) = params(&scene, Some(id), QualityPreset::High);
    assert!(p.vignette[0] > 0.0 && p.lens[0] > 0.0 && p.lens[1] > 0.0);
    p.tonemap_only();
    assert_eq!((p.vignette[0], p.lens[0], p.lens[1]), (0.0, 0.0, 0.0));
    assert_eq!(p.color, PostParams::default().color, "neutral grade");
}
