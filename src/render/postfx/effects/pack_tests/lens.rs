//! The lens effects (#1439): what each packs, its clamps, and what a display-only
//! view drops.

use super::*;

/// Each lens effect packs its knobs when on, and a zero intensity (which the
/// composite skips) when off or forbidden by the player.
#[test]
fn lens_effects_pack_when_on_and_zero_when_off_or_forbidden() {
    let mut on = vc();
    on.vignette.active = true;
    on.vignette.center = [0.25, 0.75];
    on.chromatic_aberration.active = true;
    on.film_grain.active = true;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let low = Default::default();
    let vignette = packed(&scene, Some(id), low, "vignette");
    assert_eq!(vignette, [[0.3, 0.2, 0.25, 0.75], [0.0; 4]]);
    assert_eq!(
        packed(&scene, Some(id), low, "chromatic_aberration"),
        [[0.3, 0.0, 0.0, 0.0]]
    );
    assert_eq!(
        packed(&scene, Some(id), low, "film_grain"),
        [[0.3, 0.8, 0.0, 0.0]]
    );
    for (option, effect) in [
        (GraphicsOption::Vignette, "vignette"),
        (GraphicsOption::ChromaticAberration, "chromatic_aberration"),
        (GraphicsOption::FilmGrain, "film_grain"),
    ] {
        let slots = packed(&scene, Some(id), forbidding(option), effect);
        assert_eq!(slots[0][0], 0.0, "{option:?} forbidden");
    }
    let (scene, id) = scene_with_vc(vc(), Some(cam()));
    for effect in ["vignette", "chromatic_aberration", "film_grain"] {
        assert_eq!(
            packed(&scene, Some(id), high(), effect)[0][0],
            0.0,
            "{effect} off"
        );
    }
}

/// A loaded file bypasses the ops' clamps, so packing clamps every lens knob (a zero
/// smoothness would make the vignette's `pow(0, 0)` a NaN).
#[test]
fn lens_knobs_are_clamped_when_packed() {
    let mut on = vc();
    (on.vignette.active, on.vignette.intensity) = (true, 5.0);
    (on.vignette.smoothness, on.vignette.color) = (0.0, [2.0, -1.0, 0.5]);
    let ca = &mut on.chromatic_aberration;
    (ca.active, ca.intensity) = (true, -1.0);
    (on.film_grain.active, on.film_grain.intensity) = (true, 3.0);
    on.film_grain.response = -2.0;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let min = crate::components::Vignette::MIN_SMOOTHNESS;
    let vignette = packed(&scene, Some(id), high(), "vignette");
    assert_eq!(vignette, [[1.0, min, 0.5, 0.5], [1.0, 0.0, 0.5, 0.0]]);
    assert_eq!(
        packed(&scene, Some(id), high(), "chromatic_aberration")[0][0],
        0.0
    );
    assert_eq!(
        packed(&scene, Some(id), high(), "film_grain"),
        [[1.0, 0.0, 0.0, 0.0]]
    );
}

/// A display-only view (a render texture with post-FX off) drops the lens effects
/// with the grade.
#[test]
fn a_display_only_view_drops_the_lens_effects() {
    let mut on = vc();
    on.vignette.active = true;
    on.chromatic_aberration.active = true;
    on.film_grain.active = true;
    let (scene, id) = scene_with_vc(on, Some(cam()));
    let guards = borrow_post_source(&scene, Some(id));
    let mut cx = EffectCx {
        post: guards.source(),
        graphics: high(),
        full: true,
    };
    let effects = ["vignette", "chromatic_aberration", "film_grain"];
    assert!(effects.iter().all(|e| slots_of(&cx, e)[0][0] > 0.0));
    cx.full = false;
    assert!(effects.iter().all(|e| slots_of(&cx, e)[0][0] == 0.0));
}
