//! Labelling on the torus: edges and corners join, a checkerboard is one island,
//! bounds and centroids wrap, and nothing depends on where the scan starts.

use super::*;

/// A `w × h` mask from a picture: `#` on, anything else off, top row first.
fn mask(rows: &[&str]) -> Image {
    let (w, h) = (rows[0].len() as u32, rows.len() as u32);
    let mut img = Image::new(Size::new(w, h));
    for (y, row) in rows.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            img.pixels_mut()[y * w as usize + x][0] = f32::from(u8::from(c == '#'));
        }
    }
    img
}

fn islands(rows: &[&str]) -> Islands {
    label(&mask(rows), |p| p[0] > 0.5)
}

#[test]
fn an_island_across_an_edge_and_a_corner_is_one() {
    let across = islands(&["#..#", "....", "....", "...."]);
    assert_eq!(across.islands.len(), 1, "left and right edges");
    let corner = islands(&["#...", "....", "....", "...#"]);
    assert_eq!(corner.islands.len(), 1, "diagonal corners");
    let apart = islands(&["#.#.", "....", "#.#.", "...."]);
    assert_eq!(apart.islands.len(), 4);
}

#[test]
fn a_box_across_an_edge_wraps() {
    let all = islands(&["#..#", "....", "....", "...."]);
    let island = all.islands[0];
    assert_eq!(island.start, [0.75, 0.0]);
    assert_eq!(island.size, [0.5, 0.25]);
    assert_eq!(island.centre(), [0.0, 0.125]);
    assert_eq!(island.centroid, [0.0, 0.125]);
}

#[test]
fn a_checkerboard_is_one_island_and_its_inverse_too() {
    let board = ["#.#.", ".#.#", "#.#.", ".#.#"];
    let all = islands(&board);
    assert_eq!(all.islands.len(), 1);
    assert_eq!(all.islands[0].size, [1.0, 1.0], "it loops both ways");
    let inverse = label(&mask(&board), |p| p[0] <= 0.5);
    assert_eq!(inverse.islands.len(), 1);
}

#[test]
fn a_band_spans_its_axis_and_keeps_the_other() {
    let band = islands(&["....", "####", "....", "...."]);
    let island = band.islands[0];
    assert_eq!((island.start, island.size), ([0.0, 0.25], [1.0, 0.25]));
    assert_eq!(island.centroid, [0.5, 0.375]);
}

#[test]
fn all_on_is_one_whole_island_and_all_off_none() {
    let full = Image::filled(8, [1.0, 0.0, 0.0, 1.0]);
    let on = label(&full, |p| p[0] > 0.5);
    assert_eq!(on.islands, vec![Island::default()]);
    assert!(label(&full, |p| p[0] > 2.0).islands.is_empty());
}

#[test]
fn nan_is_off() {
    let nan = Image::filled(4, [f32::NAN, 0.0, 0.0, 1.0]);
    assert!(label(&nan, |p| p[0] > 0.5).islands.is_empty());
}

#[test]
fn what_an_island_reports_does_not_depend_on_where_the_scan_meets_it() {
    // The same ring, met at its top-left pixel and, rolled, partway round.
    let a = islands(&[".....", ".###.", ".#.#.", ".###.", "....."]);
    let b = islands(&[".#..#", "##..#", ".....", ".....", "##..#"]);
    let (p, q) = (a.islands[0], b.islands[0]);
    let shift = |v: [f32; 2]| v.map(|c| (c - 0.4f32).rem_euclid(1.0));
    let near = |u: [f32; 2], v: [f32; 2]| (0..2).all(|i| (u[i] - v[i]).abs() < 1e-6);
    assert!(near(shift(p.start), q.start), "{p:?} {q:?}");
    assert!(near(shift(p.centroid), q.centroid), "{p:?} {q:?}");
    assert_eq!(p.size, q.size);
}

#[test]
fn every_pixel_reads_its_own_island() {
    let all = islands(&["#..#", "....", ".##.", "...."]);
    assert_eq!(all.islands.len(), 2);
    assert_eq!(all.at(0, 0), all.at(3, 0));
    assert_ne!(all.at(0, 0), all.at(1, 2));
    assert_eq!(all.at(1, 1), None);
}

/// Whether two islands draw apart on two channels and seeds.
fn apart(a: &Island, b: &Island) -> bool {
    a.random(0, 0) != b.random(0, 0) && a.random(1, 9) != b.random(1, 9)
}

#[test]
fn islands_sharing_a_cell_read_apart() {
    let (o, d, x) = ("#####..", "#...#..", "#.#.#..");
    let all = islands(&[o, d, x, d, o, ".......", "......."]);
    assert_eq!(all.islands.len(), 2);
    let [a, b] = [all.islands[0], all.islands[1]];
    assert_eq!((a.centroid == b.centroid, a.rank, b.rank), (true, 0, 1));
    assert!(apart(&a, &b), "a ring and the stone at its centre");
    // Two diagonal stripes, x + y ≡ 0 (mod 4): both span the tile, centroid 0.5.
    let stripes = Image::fill_uv(8, |u, v| {
        let (x, y) = ((u * 8.0) as u32, (v * 8.0) as u32);
        [f32::from(u8::from((x + y) % 4 == 0)), 0.0, 0.0, 1.0]
    });
    let all = label(&stripes, |p| p[0] > 0.5);
    assert_eq!(all.islands.len(), 2);
    assert!(apart(&all.islands[0], &all.islands[1]), "stripes");
}

#[test]
fn a_dense_mask_gives_every_island_its_own_value() {
    // Thresholded white noise at 512²: thousands of specks, several a 4 px cell.
    let noise = Image::fill_uv(512, |u, v| {
        let (x, y) = ((u * 512.0) as i64, (v * 512.0) as i64);
        [hash::unit2(x, y, 0, 11), 0.0, 0.0, 1.0]
    });
    let all = label(&noise, |p| p[0] > 0.8);
    assert!(
        all.islands.iter().any(|i| i.rank >= 3),
        "cells hold several"
    );
    let mut g: Vec<u32> = all
        .islands
        .iter()
        .map(|i| i.random(0, 0).to_bits())
        .collect();
    g.sort_unstable();
    g.dedup();
    // Only chance collisions of a 24-bit value: about n² / 2²⁵, under 0.1%.
    let n = all.islands.len();
    assert!(g.len() * 1000 >= n * 999, "{} of {n}", g.len());
}

#[test]
fn seed_zero_at_cell_zero_is_not_the_gaps_zero() {
    let mut img = Image::new(Size::new(256, 256));
    img.pixels_mut()[0][0] = 1.0;
    let speck = label(&img, |p| p[0] > 0.5).islands[0];
    assert_eq!(speck.cell(), [0, 0]);
    assert_ne!(speck.random(0, 0), 0.0);
}
