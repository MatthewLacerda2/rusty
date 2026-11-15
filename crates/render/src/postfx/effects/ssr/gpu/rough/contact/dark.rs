//! A dark object's contact reflection in front of a bright background (#2249): the
//! cube a twelfth as bright, the far wall glowing. The radius field is a mean by
//! count, so the cube's foot, many dark hits of its own, keeps its own blur
//! whatever shines behind it; a field that weighed hits by their brightness let
//! the wall's long rays loosen it. Measured: 0.95 on lavapipe with the field as
//! it is, 0.69 with the field weighted by brightness.

use super::{kept, Case, TIGHT};

#[test]
fn gpu_a_dark_cubes_contact_reflection_stays_tight_before_a_glowing_wall() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(super::W, super::H) else {
        return;
    };
    let case = Case {
        roughness: 0.35,
        thin: false,
        dark: true,
    };
    let (share, open) = kept(&mut r, case);
    eprintln!("[ssr] a dark cube on a roughness-0.35 floor before a glowing wall: its reflection under its foot adds {open} red, keeping {share:.2} of it with the wall's faded rays around it");
    assert!(open > 0, "the floor reflects the dark cube's foot");
    assert!(
        share >= TIGHT,
        "a dark contact reflection takes a bright background's blur: it keeps {share:.2}"
    );
}
