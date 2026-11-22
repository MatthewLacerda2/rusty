//! `scatter`'s behaviour: it tiles, the buckets lose no stamp, raising
//! `amount` keeps the stamps already there, and `density`, `spacing` and several
//! stamp inputs do what they say.

use super::*;
use crate::recipe::Param;

fn disc(size: u32) -> Image {
    Image::fill_uv(size, |u, v| {
        let d = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
        let r = f32::from(d < 0.4);
        [r, 0.5 * r, 0.0, 1.0]
    })
}

fn settings(p: &Params) -> Settings<'static> {
    let links: &'static Links<'static> = Box::leak(Box::default());
    Settings::of(p, links, 5)
}

fn busy() -> Params {
    Params {
        scale: Param::Num(0.3),
        scale_jitter: 0.5,
        rotation_jitter: 1.0,
        ..Params::new(24.0)
    }
}

fn brute(inputs: &[&Image], stamps: &[Stamp]) -> Image {
    Image::fill_uv(inputs[0].size(), |u, v| {
        let mut out: Rgba = [0.0, 0.0, 0.0, 1.0];
        for st in stamps {
            for (sx, sy) in (-1..=1).flat_map(|y| (-1..=1).map(move |x| (x, y))) {
                let (su, sv) = st.local(u + sx as f32, v + sy as f32);
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let p = inputs[st.pick].sample_bilinear_wrapped(su, sv);
                    fold(TileBlend::Max, &mut out, fill(p, st.value, false));
                }
            }
        }
        out
    })
}

#[test]
fn the_buckets_lose_no_stamp_and_a_stamp_crosses_the_wrap() {
    let img = disc(64);
    let s = settings(&busy());
    let stamps = stamps(&s, 1);
    assert!(
        stamps.iter().any(|st| st.centre[0] < st.half_width()),
        "one crosses"
    );
    assert!(scatter(&[&img], &s) == brute(&[&img], &stamps));
}

#[test]
fn a_bake_with_its_stamps_moved_is_the_bake_rolled() {
    let img = disc(64);
    let s = settings(&busy());
    let base = stamps(&s, 1);
    let (kx, ky) = (13, 40);
    let moved: Vec<Stamp> = base
        .iter()
        .map(|st| Stamp {
            centre: [
                (st.centre[0] + kx as f32 / 64.0).rem_euclid(1.0),
                (st.centre[1] + ky as f32 / 64.0).rem_euclid(1.0),
            ],
            ..*st
        })
        .collect();
    let (a, b) = (draw(&[&img], &base, &s), draw(&[&img], &moved, &s));
    let mut off = 0;
    for (x, y) in (0..64).flat_map(|y| (0..64).map(move |x| (x, y))) {
        off += usize::from(a.get_wrapped(x, y) != b.get_wrapped(x + kx, y + ky));
    }
    // A texel on a stamp's very edge may round to the other side once moved; a
    // seam would cut whole stamps.
    assert!(off < 100, "{off} of 4096 texels differ");
}

#[test]
fn one_more_stamp_keeps_every_texel_it_does_not_cover() {
    let img = disc(64);
    for spacing in [0.0, 1.0] {
        let p = |n| Params {
            scale: Param::Num(0.08),
            spacing,
            ..Params::new(n)
        };
        let (n, n1) = (settings(&p(20.0)), settings(&p(21.0)));
        let last = *stamps(&n1, 1).last().expect("stamp 20 has room");
        let (a, b) = (scatter(&[&img], &n), scatter(&[&img], &n1));
        let mut kept = 0;
        for (i, (pa, pb)) in a.pixels().iter().zip(b.pixels()).enumerate() {
            let (u, v) = (
                ((i % 64) as f32 + 0.5) / 64.0,
                ((i / 64) as f32 + 0.5) / 64.0,
            );
            let covers = (-1..=1).any(|sy| {
                (-1..=1).any(|sx| {
                    let (su, sv) = last.local(u + sx as f32, v + sy as f32);
                    (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv)
                })
            });
            if !covers {
                assert_eq!(pa, pb, "spacing {spacing}: texel {i}");
                kept += 1;
            }
        }
        assert!(kept > 4000 && kept < 4096, "spacing {spacing}: {kept}");
    }
}

#[test]
fn density_linked_to_a_half_black_mask_leaves_that_half_empty() {
    let mask = Image::fill_uv(64, |u, _| [f32::from(u >= 0.5), 0.0, 0.0, 1.0]);
    let s = Settings {
        density: Field::Map {
            img: &mask,
            min: 0.0,
            max: 1.0,
        },
        ..settings(&Params {
            scale: Param::Num(0.05),
            ..Params::new(200.0)
        })
    };
    let drawn = stamps(&s, 1);
    assert!(drawn.len() > 60 && drawn.len() < 140, "{}", drawn.len());
    // Bilinear reads blend the mask's edges a texel wide (and wrap at u = 0).
    let px = 1.0 / 64.0;
    assert!(drawn
        .iter()
        .all(|st| !(px..0.5 - px).contains(&st.centre[0])));
    let out = scatter(&[&disc(64)], &s);
    for (i, p) in out.pixels().iter().enumerate() {
        let u = ((i % 64) as f32 + 0.5) / 64.0;
        if (0.1..0.4).contains(&u) {
            assert_eq!(p[0], 0.0, "texel {i}");
        }
    }
}

fn closest(placed: &[[f32; 2]]) -> f32 {
    let d = |a: f32, b: f32| (a - b).abs().min(1.0 - (a - b).abs());
    let mut best = f32::INFINITY;
    for (i, a) in placed.iter().enumerate() {
        for b in &placed[..i] {
            best = best.min((d(a[0], b[0]).powi(2) + d(a[1], b[1]).powi(2)).sqrt());
        }
    }
    best
}

#[test]
fn full_spacing_keeps_every_centre_its_gap_apart() {
    for (size, want) in [(0.03, 0.03), (0.1, (0.5f32 / 300.0).sqrt())] {
        let gap = place::gap(1.0, 300, size);
        assert_eq!(gap, want);
        let placed: Vec<_> = place::place(300, gap, 9).into_iter().flatten().collect();
        assert!(placed.len() >= 285, "{size}: {} find room", placed.len());
        assert!(closest(&placed) >= gap, "{size}: {}", closest(&placed));
    }
    let random: Vec<_> = place::place(300, 0.0, 9).into_iter().flatten().collect();
    assert_eq!(random.len(), 300, "no spacing drops none");
    assert!(closest(&random) < 0.01, "pure random clumps");
}

#[test]
fn with_two_stamp_inputs_both_appear() {
    let red = Image::fill_uv(32, |_, _| [1.0, 0.0, 0.0, 1.0]);
    let green = Image::fill_uv(32, |_, _| [0.0, 1.0, 0.0, 1.0]);
    let out = scatter(&[&red, &green], &settings(&Params::new(16.0)));
    let has = |c: usize| out.pixels().iter().any(|p| p[c] == 1.0 && p[1 - c] == 0.0);
    assert!(has(0) && has(1));
}

#[test]
fn no_stamps_bake_black_and_bad_numbers_clamp() {
    let img = disc(16);
    let black = Image::new(16);
    for amount in [0.0, -3.0] {
        assert!(scatter(&[&img], &settings(&Params::new(amount))) == black);
    }
    let nan = settings(&Params::new(f32::NAN));
    assert_eq!(nan.amount, 32, "a NaN amount reads its default");
    assert!(scatter(&[&img], &nan) == scatter(&[&img], &settings(&Params::new(32.0))));
    let huge = Params {
        scale: Param::Num(f32::INFINITY),
        spacing: f32::NAN,
        ..Params::new(1e30)
    };
    let s = settings(&huge);
    assert_eq!((s.amount, s.spacing), (65_536, 0.0));
    let one = Settings {
        amount: 1,
        scale_jitter: 1.0,
        ..s
    };
    let reach = stamps(&one, 1)[0].half_width();
    assert!(
        reach > 0.0 && reach <= 0.75 * std::f32::consts::SQRT_2,
        "{reach}"
    );
    let zero = Settings {
        scale: Field::Const(0.0),
        ..settings(&Params::new(4.0))
    };
    assert!(scatter(&[&img], &zero) == black, "a size of 0 draws none");
}
