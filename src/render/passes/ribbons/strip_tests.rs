use glam::Vec3;

use super::*;
use crate::core::curve::{Curve, Gradient};

fn style() -> RibbonStyle {
    RibbonStyle {
        width: Curve::linear(1.0, 0.0),
        color: Gradient::linear([1.0, 0.0, 0.0, 1.0], [0.0, 0.0, 1.0, 0.0]),
        ..RibbonStyle::default()
    }
}

fn run(points: &[Vec3], looping: bool, style: &RibbonStyle) -> (Vec<RibbonVertex>, Vec<u32>) {
    let (mut v, mut i) = (Vec::new(), Vec::new());
    let n = build(points, looping, style, &mut v, &mut i);
    assert_eq!(n as usize, i.len());
    (v, i)
}

#[test]
fn width_colour_and_u_run_along_the_length() {
    let pts = [Vec3::ZERO, Vec3::X, Vec3::X * 4.0];
    let (v, i) = run(&pts, false, &style());
    assert_eq!((v.len(), i.len()), (6, 12), "two segments");
    // Point 1 sits a quarter of the way along.
    assert_eq!(v[2].width, 0.75);
    assert_eq!(v[2].color, [0.75, 0.0, 0.25, 0.75]);
    assert_eq!((v[2].u, v[4].u), (0.25, 1.0));
    assert_eq!((v[2].side, v[3].side), (-1.0, 1.0));
    assert_eq!(v[0].tangent, [1.0, 0.0, 0.0]);
    assert_eq!(&i[..6], &[0, 1, 2, 1, 3, 2]);
}

#[test]
fn tile_mode_runs_u_in_world_units() {
    let mut s = style();
    s.texture_mode = TextureMode::Tile;
    let (v, _) = run(&[Vec3::ZERO, Vec3::X * 3.0], false, &s);
    assert_eq!(v[2].u, 3.0);
}

#[test]
fn duplicates_merge_and_too_few_points_draw_nothing() {
    let (v, i) = run(&[Vec3::ONE, Vec3::ONE, Vec3::NAN], false, &style());
    assert!(v.is_empty() && i.is_empty());
    let (v, _) = run(&[Vec3::ZERO, Vec3::ZERO, Vec3::Y], false, &style());
    assert_eq!(v.len(), 4, "the repeated point merged");
}

#[test]
fn a_loop_closes_and_its_seam_tangent_wraps() {
    let square = [Vec3::ZERO, Vec3::X, Vec3::new(1.0, 0.0, 1.0), Vec3::Z];
    let (v, i) = run(&square, true, &style());
    assert_eq!(
        (v.len(), i.len()),
        (10, 24),
        "four segments, seam point repeated"
    );
    // At the seam the tangent runs from the last corner to the second: (1,0,-1).
    let t = Vec3::from(v[0].tangent);
    assert!((t - Vec3::new(1.0, 0.0, -1.0).normalize()).length() < 1e-6);
    assert_eq!(v[0].tangent, v[8].tangent, "both ends of the seam agree");
}

#[test]
fn indices_offset_past_existing_vertices() {
    let (mut v, mut i) = (Vec::new(), Vec::new());
    build(&[Vec3::ZERO, Vec3::X], false, &style(), &mut v, &mut i);
    build(&[Vec3::ZERO, Vec3::Y], false, &style(), &mut v, &mut i);
    assert_eq!(&i[6..], &[4, 5, 6, 5, 7, 6]);
}
