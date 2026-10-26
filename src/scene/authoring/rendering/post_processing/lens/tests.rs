//! The lens effects' ops: their clamps, and a vector with a non-finite lane kept.

use super::*;
use crate::scene::authoring::default_post_processing;

#[test]
fn lens_ops_clamp_and_refuse_non_finite_vectors() {
    let mut e = default_post_processing();
    vignette::set_intensity(&mut e, 4.0);
    vignette::set_smoothness(&mut e, -1.0);
    vignette::set_color(&mut e, [0.5, f32::NAN, 0.0]);
    vignette::set_center(&mut e, [2.0, -1.0]);
    chromatic_aberration::set_intensity(&mut e, f32::INFINITY);
    film_grain::set_intensity(&mut e, -2.0);
    film_grain::set_response(&mut e, 0.25);
    assert_eq!((e.vignette.intensity, e.vignette.smoothness), (1.0, 0.01));
    assert_eq!(
        e.vignette.color, [0.0; 3],
        "a NaN colour keeps the current one"
    );
    assert_eq!(e.vignette.center, [1.0, 0.0]);
    assert_eq!(e.chromatic_aberration.intensity, 0.0, "infinity is refused");
    assert_eq!((e.film_grain.intensity, e.film_grain.response), (0.0, 0.25));
}
