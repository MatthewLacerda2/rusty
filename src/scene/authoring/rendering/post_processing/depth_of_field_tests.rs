//! The depth of field's ops: their clamps, the Gaussian ends kept apart, and its
//! knobs by path.

use super::depth_of_field::*;
use super::knob;
use crate::components::DepthOfField;
use crate::scene::authoring::default_post_processing;

#[test]
fn dof_ops_clamp_and_keep_the_gaussian_ends_independent() {
    let mut e = default_post_processing();
    // A tween of the start out past the end and back leaves the end authored.
    set_gaussian_start(&mut e, 40.0);
    set_gaussian_start(&mut e, 10.0);
    assert_eq!(e.depth_of_field.gaussian_end, 30.0, "the end stays put");
    set_gaussian_end(&mut e, 5.0);
    assert_eq!(
        e.depth_of_field.gaussian_end, 5.0,
        "nor floors at the start"
    );
    set_gaussian_end(&mut e, -1.0);
    assert_eq!(e.depth_of_field.gaussian_end, 0.0, "but not below zero");
    set_gaussian_max_radius(&mut e, 9.0);
    set_aperture(&mut e, f32::NAN);
    set_focal_length(&mut e, 1000.0);
    set_focus_distance(&mut e, -3.0);
    let d = e.depth_of_field;
    assert_eq!((d.gaussian_max_radius, d.aperture), (1.5, 1.0));
    assert_eq!((d.focal_length, d.focus_distance), (300.0, 0.1));
    set_focus_distance(&mut e, f32::INFINITY);
    let far = DepthOfField::MAX_FOCUS_DISTANCE;
    assert_eq!(e.depth_of_field.focus_distance, far, "+inf is infinity");
    let knob = knob("depth_of_field.focus_distance").unwrap();
    (knob.set)(&mut e, 2.5);
    assert_eq!((knob.get)(&e), 2.5);
    assert!(super::knob("depth_of_field.mode").is_none(), "numbers only");
}
