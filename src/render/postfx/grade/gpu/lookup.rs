//! The Color Lookup on lavapipe (#1441): an identity `.cube` leaves the image
//! byte-identical, a channel-swap LUT swaps red and blue, a malformed one grades
//! neutral, and the bake follows the path, the contribution and the file.

use std::path::Path;

use super::{chain, Chain, W};
use crate::asset::lut::tests::{cube_of, identity_cube};
use crate::core::fs::written::contract::{follows_rebakes, Bake};
use crate::core::fs::written::write;
use crate::render::postfx::grade::{GradeParams, Lookup};
use crate::scene::authoring::default_post_processing;

/// A colour card: black, white, greys and saturated colours, in scene-linear HDR.
fn card() -> Vec<[f32; 3]> {
    (0..W)
        .map(|i| {
            let t = i as f32 / (W - 1) as f32;
            [t * 1.5, (i % 7) as f32 / 6.0, (1.0 - t) * 0.8]
        })
        .collect()
}

fn lookup(path: &str, contribution: f32) -> Option<Lookup<'_>> {
    Some(Lookup { path, contribution })
}

#[test]
fn gpu_an_identity_lut_is_byte_identical_and_a_swap_swaps_red_and_blue() {
    let Some(mut c) = chain() else { return };
    let dir = crate::test_temp::dir();
    let (id, swap) = (dir.join("id.cube"), dir.join("swap.cube"));
    std::fs::write(&id, identity_cube(17)).unwrap();
    std::fs::write(&swap, cube_of(17, |[r, g, b]| [b, g, r])).unwrap();
    let grade = GradeParams::from_volume(Some(&default_post_processing()));
    let row = card();
    let plain = c.run(&row, 0.0, &grade);
    let (id, swap) = (id.to_string_lossy(), swap.to_string_lossy());
    assert_eq!(c.run_with(&row, 0.0, &grade, lookup(&id, 1.0)), plain);
    let swapped = c.run_with(&row, 0.0, &grade, lookup(&swap, 1.0));
    for (p, s) in plain.iter().zip(&swapped) {
        let off = [
            p[2].abs_diff(s[0]),
            p[1].abs_diff(s[1]),
            p[0].abs_diff(s[2]),
        ];
        assert!(off.iter().all(|&d| d <= 1), "{p:?} swapped is {s:?}");
    }
    assert!(
        plain.iter().any(|p| p[0].abs_diff(p[2]) > 50),
        "the card has colour"
    );
    // No contribution is no LUT.
    assert_eq!(c.run_with(&row, 0.0, &grade, lookup(&swap, 0.0)), plain);
}

#[test]
fn no_contribution_names_no_lookup() {
    let mut vc = default_post_processing();
    vc.color_lookup.lut = Some("assets/luts/warm.cube".into());
    assert!(Lookup::of(Some(&vc)).is_some());
    vc.color_lookup.contribution = 0.0;
    assert_eq!(
        Lookup::of(Some(&vc)),
        None,
        "nothing read, uploaded or logged"
    );
}

#[test]
fn gpu_a_malformed_or_missing_lut_grades_neutral() {
    let Some(mut c) = chain() else { return };
    let dir = crate::test_temp::dir();
    let bad = dir.join("short.cube");
    let mut text = identity_cube(4);
    text.truncate(text.trim_end().rfind('\n').unwrap());
    std::fs::write(&bad, text).unwrap();
    let grade = GradeParams::from_volume(Some(&default_post_processing()));
    let row = card();
    let plain = c.run(&row, 0.0, &grade);
    let bad = bad.to_string_lossy();
    assert_eq!(c.run_with(&row, 0.0, &grade, lookup(&bad, 1.0)), plain);
    let missing = dir.join("absent.cube");
    let missing = missing.to_string_lossy();
    assert_eq!(c.run_with(&row, 0.0, &grade, lookup(&missing, 1.0)), plain);
}

#[test]
fn gpu_the_bake_follows_the_contribution_the_path_and_the_file() {
    let Some(mut c) = chain() else { return };
    let dir = crate::test_temp::dir();
    let (a, b) = (dir.join("a.cube"), dir.join("b.cube"));
    std::fs::write(&a, identity_cube(2)).unwrap();
    std::fs::write(&b, identity_cube(2)).unwrap();
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    let grade = GradeParams::NEUTRAL;
    let bake = |c: &mut Chain, l| {
        let gpu = (&c.r.device, &c.r.queue);
        c.pfx.grade.prepare(gpu, &mut c.r.grade_bake, &grade, l)
    };
    assert!(bake(&mut c, lookup(&a, 1.0)), "the first bake");
    assert!(!bake(&mut c, lookup(&a, 1.0)), "a still lookup bakes once");
    assert!(bake(&mut c, lookup(&a, 0.5)), "the contribution");
    let other = bake(&mut c, lookup(&b, 0.5));
    assert!(other, "another file of the same size and domain");
    write(Path::new(b.as_ref()), identity_cube(3).as_bytes()).unwrap();
    assert!(
        !bake(&mut c, lookup(&b, 0.5)),
        "read again next frame, not mid-frame"
    );
    c.r.grade_bake.luts.refresh();
    assert!(bake(&mut c, lookup(&b, 0.5)), "the file written over");
    assert!(bake(&mut c, None), "the lookup cleared");
}

#[test]
fn gpu_the_lut_cache_follows_rebakes() {
    let Some(mut c) = chain() else { return };
    let path = crate::test_temp::dir().join("baked.cube");
    let domain = |bake| match bake {
        Bake::A => 1.0,
        Bake::B => 2.0,
    };
    follows_rebakes(
        &path.to_string_lossy(),
        |p, bake| {
            let text = format!("DOMAIN_MAX {0} {0} {0}\n", domain(bake)) + &identity_cube(2);
            write(Path::new(p), text.as_bytes()).unwrap();
        },
        |p| {
            let luts = &mut c.r.grade_bake.luts;
            luts.refresh();
            luts.get(&c.r.device, &c.r.queue, p).map(|l| l.domain[1][0])
        },
        |max, bake| *max == Some(domain(bake)),
    );
}
