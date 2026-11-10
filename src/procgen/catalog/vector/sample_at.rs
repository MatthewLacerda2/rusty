//! The `sample_at` op: read a map at coordinates other maps give (#2002).

use crate::procgen::catalog::{Cx, Op, Row};
use crate::procgen::image_buf::Image;
use crate::procgen::ops::sample::{sample_at, Coords};
use crate::procgen::recipe::{arity, op, OpInfo};

/// Sample input A at the domain point each pixel's coordinates name: with two
/// inputs, input B's red is `u` and its green `v` (what `coordinates` writes); with
/// three, B's red is `u` and C's red is `v`. Wrapped, and filtered by each pixel's
/// footprint through a mip chain, so a remap that shrinks A reads its average, not
/// one aliased tap.
pub struct SampleAt;

impl Row for SampleAt {
    const INFO: OpInfo = op("sample_at", "vector", arity(2, 3), &[]);
}

impl Op for SampleAt {
    fn run(&self, cx: &Cx) -> Image {
        let coords = match cx.optional(2) {
            Some(v) => Coords::Split(cx.input(1), v),
            None => Coords::Packed(cx.input(1)),
        };
        sample_at(cx.input(0), coords)
    }
}
