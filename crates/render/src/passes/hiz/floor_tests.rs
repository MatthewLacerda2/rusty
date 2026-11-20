//! Who culls by Hi-Z (#2338), with no adapter: the instance floor, and the
//! preset, camera and capture rules beside it.

use super::{culls, draws_enough, MIN_INSTANCES};

/// No batch drawn by cluster, or one.
fn no() -> bool {
    false
}
fn yes() -> bool {
    true
}

#[test]
fn a_camera_culls_from_the_floor_up() {
    let floor = MIN_INSTANCES as usize;
    assert!(!draws_enough((floor - 1, no), MIN_INSTANCES));
    assert!(draws_enough((floor, no), MIN_INSTANCES));
    assert!(draws_enough((floor + 1, no), MIN_INSTANCES));
    assert!(
        !draws_enough((0, no), MIN_INSTANCES),
        "a camera testing nothing"
    );
    assert!(
        draws_enough((1, no), 0),
        "a lowered floor culls a single box"
    );
}

#[test]
fn a_camera_drawing_by_cluster_culls_under_the_floor() {
    assert!(draws_enough((1, yes), MIN_INSTANCES), "one clustered shell");
    assert!(draws_enough((0, yes), MIN_INSTANCES));
}

#[test]
fn a_camera_at_the_floor_never_walks_its_batches_for_clusters() {
    let walked = || -> bool { panic!("asked for clusters above the floor") };
    assert!(draws_enough(
        (MIN_INSTANCES as usize, walked),
        MIN_INSTANCES
    ));
}

#[test]
fn the_preset_the_camera_and_a_capture_still_decide() {
    assert!(culls(true, (true, true), false));
    assert!(
        !culls(false, (true, true), false),
        "no indirect first instance"
    );
    assert!(!culls(true, (false, true), false), "the preset off");
    assert!(!culls(true, (true, false), false), "the camera's flag off");
    assert!(!culls(true, (true, true), true), "during a capture");
}
