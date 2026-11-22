use proptest::prelude::*;
use proptest::test_runner::{Config, TestRunner};

use super::equivalence_ops::{edit, library, r_op, R};
use super::fixture::{fresh, held, table_holds, yard, RES};
use super::occlusion::ops::{self as occlusion_ops, baked_yard};
use super::shadow_ops;
use crate::Renderer;

fn holds(r: &mut Renderer, ops: &[R]) -> Result<(), TestCaseError> {
    let (mut scene, mut other) = (baked_yard(6), yard(3));
    library(&mut other, 1);
    r.capture.static_only = false;
    r.draw_poses.clear();
    r.extract_scene(&scene);
    for op in ops.iter().chain([&R::Frame]) {
        edit(r, &mut scene, op);
        if let R::Other(c) = *op {
            library(&mut other, c);
        }
        if matches!(op, R::Frame) {
            r.extract_scene(&scene);
            r.extract_scene(&other);
            for s in [&scene, &other] {
                let kept = held(r, s);
                prop_assert_eq!(kept, fresh(r, s), "after {:?}", ops);
            }
            let table = table_holds(r);
            prop_assert!(table.is_ok(), "{:?} after {:?}", table, ops);
            let shadows = shadow_ops::kept_equals_fresh(r, &scene)
                .and_then(|()| occlusion_ops::kept_equals_fresh(r, &scene))
                .and_then(|()| r.rays_kept_equal_fresh(&scene));
            prop_assert!(shadows.is_ok(), "{:?} after {:?}", shadows, ops);
        }
    }
    Ok(())
}

#[test]
fn gpu_any_edit_sequence_extracts_what_a_fresh_build_holds() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    let mut runner = TestRunner::new(Config {
        cases: 48,
        ..Config::default()
    });
    let ops = proptest::collection::vec(r_op(), 1..40);
    let r = std::cell::RefCell::new(&mut r);
    let result = runner.run(&ops, |ops| holds(&mut r.borrow_mut(), &ops));
    result.unwrap_or_else(|e| panic!("{e}"));
}
