//! Who culls by Hi-Z (#2338), with no adapter: the instance floor, and the
//! preset, camera and capture rules beside it.

use super::{culls, draws_enough, MIN_INSTANCES};

#[test]
fn a_camera_culls_from_the_floor_up() {
    assert!(!draws_enough(MIN_INSTANCES - 1, MIN_INSTANCES));
    assert!(draws_enough(MIN_INSTANCES, MIN_INSTANCES));
    assert!(draws_enough(MIN_INSTANCES + 1, MIN_INSTANCES));
    assert!(!draws_enough(0, MIN_INSTANCES), "a camera drawing nothing");
    assert!(draws_enough(1, 0), "a lowered floor culls a single box");
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
