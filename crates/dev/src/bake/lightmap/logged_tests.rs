use std::cell::{Cell, RefCell};
use std::time::Duration;

use super::logged::{bake_logged, read, watch};
use super::tests::{quick, scene, scene_path};
use super::*;
use crate::bake::progress::Line;
use rusty_lighting::lightmap::BakeProgress;

#[test]
fn a_logged_bake_is_byte_identical_to_an_unlogged_one() {
    let (scene, ..) = scene();
    let input = gather_bake_input(&scene);
    let path = scene_path("logged_bytes");
    let plain = bake_logged(&input, &quick(), &mut BakeLog::off(), &path).unwrap();
    let (plain, untimed) = (plain.pages, plain.measured.times);
    let now = Cell::new(Duration::ZERO);
    let lines: RefCell<Vec<Line>> = RefCell::default();
    let clock = || {
        now.set(now.get() + Duration::from_secs(60));
        now.get()
    };
    let mut log = BakeLog::new(clock, |l| lines.borrow_mut().push(l));
    let logged = bake_logged(&input, &quick(), &mut log, &path).unwrap();
    let (logged, times) = (logged.pages, logged.measured.times);
    drop(log);
    assert!(plain.maps > 0);
    assert_eq!(logged, plain, "a log never changes a byte");
    assert_eq!((times.len(), untimed.len()), (plain.maps, plain.maps));
    assert!(
        times.iter().all(|t| t.texels > 0 && !t.busy.is_zero()),
        "{times:?}"
    );
    let lines = lines.into_inner();
    assert!(lines[0].milestone, "the step's first line is a milestone");
    let pages = plain.colour.len() + plain.directions.len();
    let last = &lines.last().expect("a line").text;
    let encoded = format!(" lightmap pages 100% ({pages}/{pages} pages encoded)");
    assert!(last.ends_with(&encoded), "{last}");
}

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
    assert_eq!(lines[0].text, "[Lighting] 0:00 lightmaps preparing");
    assert!(lines[0].milestone, "the step's start stays in the console");
    let last = &lines.last().expect("a line").text;
    assert_eq!(
        last,
        "[Lighting] 0:00 lightmap pages 100% (1/1 pages encoded)"
    );
}
