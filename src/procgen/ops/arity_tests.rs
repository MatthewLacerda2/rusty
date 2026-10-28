//! The catalog's input arity is what [`eval_node`] reads (#1485): every op runs with
//! its fewest and its most inputs, panics with one fewer than its fewest, and reads
//! its last optional input.

use std::panic::{catch_unwind, AssertUnwindSafe};

use super::*;
use crate::procgen::recipe::{sample_op, OPS};

const RES: u32 = 8;

/// `n` distinct white-noise images.
fn images(n: usize) -> Vec<Image> {
    (0..n as u64)
        .map(|seed| eval_node(&OpKind::WhiteNoise, &[], RES, seed))
        .collect()
}

fn eval(op: &OpKind, inputs: &[Image]) -> Image {
    let refs: Vec<&Image> = inputs.iter().collect();
    eval_node(op, &refs, RES, 0)
}

#[test]
fn every_op_runs_on_its_arity_and_needs_its_minimum() {
    for info in OPS {
        let (op, arity) = (sample_op(info), info.inputs);
        let (min, max) = (usize::from(arity.min), usize::from(arity.max));
        eval(&op, &images(max));
        eval(&op, &images(min));
        // `combine_rgb` reads a missing channel as 0, so it runs on none; its minimum
        // of 1 is policy (an all-black RGB is never what a recipe meant), not need.
        if min > 0 && info.op != "combine_rgb" {
            let short = catch_unwind(AssertUnwindSafe(|| eval(&op, &images(min - 1))));
            assert!(short.is_err(), "{} ran with {} inputs", info.op, min - 1);
        }
    }
}

#[test]
fn every_optional_input_is_read() {
    for info in OPS.iter().filter(|info| info.inputs.max > info.inputs.min) {
        let op = sample_op(info);
        let mut inputs = images(usize::from(info.inputs.max));
        let before = eval(&op, &inputs);
        let last = inputs.len() - 1;
        inputs[last] = eval_node(&OpKind::WhiteNoise, &[], RES, 99);
        assert_ne!(
            before,
            eval(&op, &inputs),
            "{} ignores input {last}",
            info.op
        );
    }
}
