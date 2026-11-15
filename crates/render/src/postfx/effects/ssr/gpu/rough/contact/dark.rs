//! A dark object's contact reflection in front of a bright background (#2249): the
//! cube a twelfth as bright, the far wall glowing. The radius field leans a texel
//! much darker than the hits around it to their radius (`ssr.wgsl`'s
//! `texel_mean`), which a cord's sliver before a lamp needs; a texel holding many
//! dark hits of its own, the cube's foot, must keep its own blur all the same.

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
