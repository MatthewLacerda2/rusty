//! A logged lightmap bake (#1667): the same bytes as an unlogged one, and lines
//! that end at every texel and mesh.

use std::cell::{Cell, RefCell};
use std::time::Duration;

use super::logged::bake_logged;
use super::tests::{quick, scene};
use super::*;
use crate::dev::bake::progress::Line;

#[test]
fn a_logged_bake_is_byte_identical_to_an_unlogged_one() {
    let (scene, ..) = scene();
    let input = gather_bake_input(&scene);
    let plain = bake_logged(&input, &quick(), &mut BakeLog::off());
    // A clock that jumps a minute per read, so every observation prints.
    let now = Cell::new(Duration::ZERO);
    let lines: RefCell<Vec<Line>> = RefCell::default();
    let clock = || {
        now.set(now.get() + Duration::from_secs(60));
        now.get()
    };
    let mut log = BakeLog::new(clock, |l| lines.borrow_mut().push(l));
    let logged = bake_logged(&input, &quick(), &mut log);
    drop(log);
    assert!(!plain.is_empty());
    assert_eq!(logged, plain, "a log never changes a texel");
    let lines = lines.into_inner();
    assert!(lines[0].milestone, "the step's first line is a milestone");
    let last = &lines.last().expect("a line").text;
    let n = plain.len();
    assert!(
        last.starts_with(&format!("[Lighting] lightmaps 100% ({n}/{n} meshes,")),
        "{last}"
    );
}
