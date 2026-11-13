//! Tests for what a run holds at its peak (#2002's review): a gray chain costs an
//! input and its output, as an RGBA one does, and a node reads each distinct input
//! once.

use super::held::Held;
use super::slots::{image_bytes, Slots};
use super::tests::chain;
use super::*;

/// One 4 × 4 RGBA f32 image: what a node of `chain` costs whole.
const IMAGE: usize = 4 * 4 * 16;

#[test]
fn a_gray_chain_peaks_at_an_input_and_its_output() {
    // `chain` is a gray constant and inverts of it: every image packs to gray.
    for (k, (scope, targets)) in [(Scope::Needed, &["n8"][..]), (Scope::Every, &[][..])]
        .into_iter()
        .enumerate()
    {
        let recipe = chain(8);
        let plan = plan(&recipe, targets, scope).unwrap();
        let run = run(&recipe, &Sources::new(), &plan, &mut |_, _| {}).unwrap();
        assert!(run.peak_bytes <= 2 * IMAGE, "scope {k}: {}", run.peak_bytes);
        assert_eq!(run.peak_live, 2);
    }
}

#[test]
fn a_repeated_input_is_rebuilt_once_and_a_last_read_one_is_moved() {
    let gray = Image::filled(4, [0.5, 0.5, 0.5, 1.0]);
    let colour = Image::filled(4, [0.1, 0.2, 0.3, 1.0]);
    let mut slots = Slots::new(2);
    slots.put(0, Held::pack(gray.clone()));
    slots.put(1, Held::pack(colour));
    // `mix {n, c, n}` with both read again later: `n` rebuilt once, `c` borrowed.
    let g = slots.gather(&[0, 1, 0], &[3, 2], &[]);
    let edges = g.edges();
    assert!(std::ptr::eq(edges[0], edges[2]));
    assert_eq!(edges[0], &gray);
    assert_eq!(g.owned(), (1, image_bytes(&gray)));
    // The last reader of `n` takes it out of its slot instead of copying it.
    let g = slots.gather(&[0, 1, 0], &[2, 2], &[]);
    assert_eq!(g.owned(), (1, image_bytes(&gray)));
    drop(g);
    assert!(slots.get(0).is_none() && slots.get(1).is_some());
}
