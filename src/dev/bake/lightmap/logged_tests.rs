//! A logged lightmap bake (#1667): the same bytes as an unlogged one, and lines
//! that end at every texel and mesh, then at every page encoded (#1694).

use std::cell::{Cell, RefCell};
use std::time::Duration;

use super::logged::{bake_logged, read, watch};
use super::tests::{quick, scene};
use super::*;
use crate::dev::bake::progress::Line;
use crate::scene::lighting::lightmap::BakeProgress;

#[test]
fn a_logged_bake_is_byte_identical_to_an_unlogged_one() {
    let (scene, ..) = scene();
    let input = gather_bake_input(&scene);
    let plain = bake_logged(&input, &quick(), &mut BakeLog::off());
    let (plain, untimed) = plain.unwrap();
    // A clock that jumps a minute per read, so every observation prints.
    let now = Cell::new(Duration::ZERO);
    let lines: RefCell<Vec<Line>> = RefCell::default();
    let clock = || {
        now.set(now.get() + Duration::from_secs(60));
        now.get()
    };
    let mut log = BakeLog::new(clock, |l| lines.borrow_mut().push(l));
    let (logged, times) = bake_logged(&input, &quick(), &mut log).unwrap();
    drop(log);
    assert!(plain.maps > 0);
    assert_eq!(logged, plain, "a log never changes a byte");
    // Only a logged bake times its meshes (#1891), one entry per lightmap.
    assert_eq!((times.len(), untimed.len()), (plain.maps, 0));
    let lines = lines.into_inner();
    assert!(lines[0].milestone, "the step's first line is a milestone");
    // The trace, then the encode, whose last line is its last page.
    let pages = plain.colour.len() + plain.directions.len();
    let last = &lines.last().expect("a line").text;
    let encoded = format!(" lightmap pages 100% ({pages}/{pages} pages encoded)");
    assert!(last.ends_with(&encoded), "{last}");
}

/// A trace done before the first poll (#1808): the worker reaches and finishes its
/// pages at once, yet the log still says the lightmap step started, first.
#[test]
fn a_bake_that_reaches_its_pages_at_once_still_says_the_lightmaps_first() {
    let lines: RefCell<Vec<Line>> = RefCell::default();
    let mut log = BakeLog::new(|| Duration::ZERO, |l| lines.borrow_mut().push(l));
    let instant = |progress: &BakeProgress| {
        progress.start_pages(1);
        progress.page_done();
        Ok(())
    };
    watch(&BakeProgress::default(), read, &mut log, instant).unwrap();
    drop(log);
    let lines = lines.into_inner();
    // No timing: whatever a poll saw in between, the first and last lines hold.
    assert_eq!(lines[0].text, "[Lighting] 0:00 lightmaps preparing");
    assert!(lines[0].milestone, "the step's start stays in the console");
    let last = &lines.last().expect("a line").text;
    assert_eq!(
        last,
        "[Lighting] 0:00 lightmap pages 100% (1/1 pages encoded)"
    );
}
