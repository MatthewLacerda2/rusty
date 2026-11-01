//! A probe stage as the bake log reads it (#1667).

use super::stage_progress;
use crate::dev::bake::steps::Stage;

#[test]
fn a_probe_step_with_nothing_to_bake_says_nothing() {
    let light = |total| {
        stage_progress(Stage::Light {
            bounce: 1,
            done: 0,
            total,
        })
    };
    let reflection = |total| stage_progress(Stage::Reflection { done: 0, total });
    assert_eq!((light(0), reflection(0)), (None, None));
    assert_eq!(
        (light(2).unwrap().total, reflection(3).unwrap().total),
        (2, 3)
    );
}
