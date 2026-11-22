//! `pool::run_made`: each chart is made once, by the first worker to reach it and never before the
//! run does; results equal `pool::run`'s on any thread count.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use super::bake::pool::{run, run_made, BATCH};
use super::*;

const SIZES: [usize; 6] = [0, 1, BATCH, BATCH + 1, 3 * BATCH + 7, 5];

fn items(c: usize) -> Vec<usize> {
    (0..SIZES[c]).map(|i| c * 1000 + i).collect()
}

#[test]
fn a_made_chart_is_made_once_and_gives_what_a_cut_one_does() {
    let trace = |c: usize, first: usize, batch: &[usize]| {
        let at = |(k, &item): (usize, &usize)| (c, first + k, item * 3);
        batch.iter().enumerate().map(at).collect::<Vec<_>>()
    };
    let finish = |c: usize, items: Vec<usize>, out: Vec<(usize, usize, usize)>| (c, items, out);
    let charts: Vec<Vec<usize>> = (0..SIZES.len()).map(items).collect();
    let cut = run(charts, 3, &BakeProgress::default(), trace, finish);
    for threads in [1, 2, 7] {
        let made: Vec<AtomicUsize> = SIZES.iter().map(|_| AtomicUsize::new(0)).collect();
        let make = |c: usize| {
            made[c].fetch_add(1, Ordering::Relaxed);
            items(c)
        };
        let progress = BakeProgress::default();
        let out = run_made(&SIZES, make, threads, &progress, trace, finish);
        assert_eq!(out, cut, "{threads} threads");
        let counts: Vec<usize> = made.iter().map(|m| m.load(Ordering::Relaxed)).collect();
        assert_eq!(counts, vec![1; SIZES.len()], "each chart made once");
        assert_eq!(progress.done(), SIZES.iter().sum::<usize>() as u64);
    }
}

#[test]
fn a_chart_is_made_only_once_the_run_reaches_it() {
    // On one worker the charts go in order, so when chart `c` is made every
    // chart before it has been finished, and none after it made.
    let log = Mutex::new(Vec::new());
    let make = |c: usize| {
        log.lock().unwrap().push(format!("make {c}"));
        items(c)
    };
    let finish =
        |c: usize, _: Vec<usize>, _: Vec<()>| log.lock().unwrap().push(format!("finish {c}"));
    let trace = |_: usize, _: usize, batch: &[usize]| vec![(); batch.len()];
    run_made(&SIZES, make, 1, &BakeProgress::default(), trace, finish).expect("ran");
    let expect: Vec<String> = (0..SIZES.len())
        .flat_map(|c| [format!("make {c}"), format!("finish {c}")])
        .collect();
    assert_eq!(log.into_inner().unwrap(), expect);
}

#[test]
fn a_cancelled_run_makes_no_chart() {
    let progress = BakeProgress::default();
    progress.cancel();
    let make = |_: usize| -> Vec<usize> { panic!("made after a cancel") };
    let out = run_made(
        &SIZES,
        make,
        4,
        &progress,
        |_, _, _| vec![0u8],
        |_, _, _| 0u8,
    );
    assert_eq!(out, None);
}
