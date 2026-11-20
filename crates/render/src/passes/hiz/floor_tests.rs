//! Who culls by Hi-Z (#2338), with no adapter: the instance floor, and the
//! preset, camera and capture rules beside it.

use super::{culls, draws_enough, MIN_INSTANCES};

#[test]
fn a_camera_culls_from_the_floor_up() {
    let floor = MIN_INSTANCES as usize;
    assert!(!draws_enough((floor - 1, false), MIN_INSTANCES));
    assert!(draws_enough((floor, false), MIN_INSTANCES));
    assert!(draws_enough((floor + 1, false), MIN_INSTANCES));
    assert!(
        !draws_enough((0, false), MIN_INSTANCES),
        "a camera testing nothing"
    );
    assert!(
        draws_enough((1, false), 0),
        "a lowered floor culls a single box"
    );
}

#[test]
fn a_camera_drawing_by_cluster_culls_under_the_floor() {
    assert!(
        draws_enough((1, true), MIN_INSTANCES),
        "one clustered shell"
    );
    assert!(draws_enough((0, true), MIN_INSTANCES));
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
