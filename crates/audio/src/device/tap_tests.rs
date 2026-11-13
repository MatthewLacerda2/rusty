//! The final-mix tap's ring (#1579): what is copied drains back in order, a full
//! ring drops whole blocks, counts them and drains them back as silence in their
//! place, and a new capture starts empty.

use super::*;

/// `frames` stereo frames, left and right both counting up from `from`.
fn block(from: usize, frames: usize) -> Vec<Frame> {
    (from..from + frames)
        .map(|i| Frame {
            left: i as f32,
            right: -(i as f32),
        })
        .collect()
}

#[test]
fn a_copied_block_drains_back_interleaved() {
    let tap = Tap::default();
    tap.copy(&block(0, 4), 1.0 / 48_000.0);
    assert!(tap.ring.get().is_none(), "nothing is copied before a start");
    tap.start();
    tap.copy(&block(1, 2), 1.0 / 48_000.0);
    let mut out = Vec::new();
    tap.drain_into(&mut out);
    assert_eq!(out, [1.0, -1.0, 2.0, -2.0]);
    assert_eq!(tap.rate(), Some(48_000));
    out.clear();
    tap.drain_into(&mut out);
    assert!(out.is_empty(), "a drain takes each sample once");
}

#[test]
fn a_full_ring_drops_whole_blocks_and_counts_them() {
    let tap = Tap::default();
    tap.start();
    let frames = RING_SAMPLES / 2;
    tap.copy(&block(0, frames - 1), 0.0);
    tap.copy(&block(0, 2), 0.0);
    tap.copy(&block(0, 1), 0.0);
    assert_eq!(
        tap.dropped_frames(),
        3,
        "a block that fits waits out the gap"
    );
    let mut out = Vec::new();
    tap.drain_into(&mut out);
    assert_eq!(
        out.len(),
        RING_SAMPLES - 2 + 3 * 2,
        "the gap drains as silence"
    );
    assert_eq!(out[RING_SAMPLES - 3], -((frames - 2) as f32));
    assert!(out[RING_SAMPLES - 2..].iter().all(|s| *s == 0.0));
    tap.copy(&block(7, 1), 0.0);
    out.clear();
    tap.drain_into(&mut out);
    assert_eq!(out, [7.0, -7.0], "drained, the ring takes blocks again");
    assert_eq!(tap.dropped_frames(), 3, "the count stays for the report");
}

#[test]
fn a_new_capture_starts_empty() {
    let tap = Tap::default();
    tap.start();
    tap.copy(&block(0, 3), 0.0);
    tap.stop();
    tap.copy(&block(9, 3), 0.0);
    tap.start();
    assert_eq!(tap.dropped_frames(), 0);
    let mut out = Vec::new();
    tap.drain_into(&mut out);
    assert!(out.is_empty(), "the last capture's leftovers are discarded");
}
