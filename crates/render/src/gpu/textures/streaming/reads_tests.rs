use std::rc::Weak;
use std::time::{Duration, Instant};

use super::gpu_tests::gradient;
use super::reads::{Reads, IN_FLIGHT};
use crate::gpu::textures::TextureKind;

fn ask(reads: &mut Reads, map: &rusty_asset::MapRef, skip: u32) {
    reads.ask(map, TextureKind::Colour, (skip, Weak::new()));
}

#[test]
fn a_nearer_target_waits_for_the_read_in_flight_and_the_cap_counts_every_read() {
    let mut reads = Reads::new(wgpu::Features::empty());
    let maps: Vec<_> = (0..6)
        .map(|i| gradient(&format!("reads_{i}"), 16))
        .collect();
    ask(&mut reads, &maps[0], 3);
    ask(&mut reads, &maps[0], 1);
    assert_eq!(reads.in_flight, 1, "one read per map");
    for map in &maps[1..] {
        ask(&mut reads, map, 2);
    }
    assert_eq!(reads.in_flight, IN_FLIGHT, "the cap holds");
    let start = Instant::now();
    let mut back = Vec::new();
    while back.len() < IN_FLIGHT {
        assert!(
            start.elapsed() < Duration::from_secs(10),
            "reads never came back"
        );
        match reads.take() {
            Some(((map, skip, _), asked)) => back.push((map, skip, asked.is_some())),
            None => std::thread::sleep(Duration::from_millis(1)),
        }
    }
    assert_eq!(reads.in_flight, 0);
    assert!(
        back.iter().all(|b| b.2),
        "every result finds what asked for it"
    );
    let first = back.iter().find(|b| b.0 == maps[0]).map(|b| b.1);
    assert_eq!(first, Some(3), "the first ask's read, not the nearer one's");
    ask(&mut reads, &maps[0], 1);
    assert_eq!(reads.in_flight, 1, "taken back: the nearer target is asked");
}
