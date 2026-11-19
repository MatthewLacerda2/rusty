//! `warp` moves nothing where `by` is mid-grey; red above it moves the content
//! towards −U (the read goes the other way) by `strength` times its distance from
//! 0.5, green the same along V; a linked `strength` is read at each pixel.

use serde_json::json;

use super::*;
use crate::catalog::filter::test_probe::{relief, rolled};
use crate::ops::eval_linked;
use crate::ops::field::Links;
use crate::recipe::OpKind;
use crate::source::Sources;

const N: i64 = 32;

fn warp(strength: Param) -> OpKind {
    OpKind::Warp(Warp { strength })
}

fn run(op: &OpKind, img: &Image, by: &Image, links: &Links) -> Image {
    eval_linked(op, &[img, by], links, img.size(), 0, &Sources::new())
}

fn run_num(strength: f32, img: &Image, by: &Image) -> Image {
    run(&warp(Param::Num(strength)), img, by, &Links::default())
}

/// A dot one pixel across at `(x, y)` of a 32² tile.
fn dot((x, y): (i64, i64)) -> Image {
    let mut img = Image::filled(N as u32, [0.0, 0.0, 0.0, 1.0]);
    img.pixels_mut()[(y.rem_euclid(N) * N + x.rem_euclid(N)) as usize] = [1.0; 4];
    img
}

#[test]
fn a_mid_grey_map_moves_nothing() {
    let (img, grey) = (relief(N as u32), Image::filled(N as u32, [0.5; 4]));
    for strength in [0.0, 0.3, -2.0, 7.5] {
        assert_eq!(run_num(strength, &img, &grey), img, "strength {strength}");
    }
}

#[test]
fn red_moves_along_u_and_green_along_v_by_strength_times_the_offset_from_grey() {
    let mark = (12, 20);
    // (strength, channel value, pixels the content moves towards −U or −V)
    let cases = [
        (0.5, 0.75, 4),
        (0.25, 1.0, 4),
        (1.0, 0.625, 4),
        (0.5, 0.25, -4),
        (-0.5, 0.75, -4),
        (1.0, 1.0, 16),
    ];
    for axis in 0..2 {
        for (strength, value, px) in cases {
            let mut push = [0.5, 0.5, 0.5, 1.0];
            push[axis] = value;
            let by = Image::filled(N as u32, push);
            let out = run_num(strength, &dot(mark), &by);
            let want = if axis == 0 {
                (mark.0 - px, mark.1)
            } else {
                (mark.0, mark.1 - px)
            };
            assert_eq!(out, dot(want), "axis {axis}, {strength} × {value}");
        }
    }
}

#[test]
fn a_linked_strength_of_zero_leaves_its_half_untouched() {
    // `strength` 0 on the left half and 1 on the right, under a uniform push of a
    // quarter tile along U.
    let half = Image::fill_uv(N as u32, |u, _| [if u < 0.5 { 0.0 } else { 1.0 }; 4]);
    let strength: Param = serde_json::from_value(json!({ "node": "m" })).unwrap();
    let links = Links::new(vec![("m", &half)]);
    let (img, by) = (
        relief(N as u32),
        Image::filled(N as u32, [0.75, 0.5, 0.5, 1.0]),
    );
    let out = run(&warp(strength), &img, &by, &links);
    let moved = rolled(&img, -N / 4, 0);
    for (i, p) in out.pixels().iter().enumerate() {
        let want = if (i as i64 % N) < N / 2 {
            img.pixels()[i]
        } else {
            moved.pixels()[i]
        };
        assert_eq!(*p, want, "pixel {i}");
    }
}
