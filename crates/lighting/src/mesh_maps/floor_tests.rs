//! The curvature band's floor is per UV island (#2292): an island packed small in
//! UV bakes wider, the others keep the radius asked for, and the bake says which
//! were raised; a radius above every floor is used exactly as asked.

use glam::Vec2;

use super::curvature::Curvature;
use super::fixtures::{cell, cube};
use super::floor::{sig, Radii, Raised, MIN_TEXELS};
use super::{Baked, MeshMap, MeshSource, Settings};
use rusty_procgen::image_buf::Image;

const SIZE: u32 = 64;

/// The unit cube with its +Z face (vertices 16..20, cell 4) packed into a UV
/// square `shrink` of its cell's side: `1 / shrink` times coarser texels there.
fn packed(shrink: f32) -> MeshSource {
    let mut m = cube();
    let (lo, _) = cell(4);
    for uv in &mut m.uvs[16..20] {
        *uv = lo + (*uv - lo) * shrink;
    }
    m
}

fn packed_cube() -> MeshSource {
    packed(0.25)
}

/// The three-texel floor at [`SIZE`] of one 1 m² cube face packed into a cell
/// scaled by `shrink` on each side.
fn cell_floor(shrink: f32) -> f32 {
    let (lo, hi) = cell(0);
    let uv = (hi - lo) * shrink;
    MIN_TEXELS * (1.0 / (uv.x * uv.y)).sqrt() / SIZE as f32
}

fn settings(radius: f32) -> Settings {
    Settings {
        curvature_radius: Some(radius),
        ..Settings::new(SIZE)
    }
}

/// The curvature of `mesh` at `radius`, and the islands the bake raised.
fn curvature(mesh: &MeshSource, radius: f32) -> (Image, Option<Raised>) {
    let mut out = None;
    let raised = super::bake(mesh, &[MeshMap::Curvature], &settings(radius), |_, img| {
        out = Some(img);
        Ok(())
    })
    .unwrap();
    (out.unwrap(), raised)
}

/// The red channel of `img` over the texels whose centres lie inside cell `k`'s
/// corner `share` of its side (1 the whole cell).
fn cell_texels(img: &Image, k: usize, share: f32) -> Vec<f32> {
    let (lo, hi) = cell(k);
    let hi = lo + (hi - lo) * share;
    let mut out = Vec::new();
    for y in 0..SIZE {
        for x in 0..SIZE {
            let uv = (Vec2::new(x as f32, y as f32) + 0.5) / SIZE as f32;
            if uv.cmpge(lo).all() && uv.cmple(hi).all() {
                out.push(img.pixels()[(y * SIZE + x) as usize][0]);
            }
        }
    }
    out
}

#[test]
fn a_radius_between_two_islands_floors_holds_on_the_roomy_one() {
    let mesh = packed_cube();
    let (big, small) = (cell_floor(1.0), cell_floor(0.25));
    assert!((small / big - 4.0).abs() < 1e-3, "{big} vs {small}");
    let between = (big + small) * 0.5;
    let (asked, raised) = curvature(&mesh, between);
    let (floored, _) = curvature(&mesh, small);
    // The +Y face (cell 0) bakes the radius asked for: a narrower band than the
    // packed face's floor gives it.
    let (a, f) = (cell_texels(&asked, 0, 1.0), cell_texels(&floored, 0, 1.0));
    let wider = |v: &[f32]| v.iter().filter(|&&r| r > 0.5 + 1e-3).count();
    assert!(wider(&a) < wider(&f), "{} vs {}", wider(&a), wider(&f));
    // The packed face is raised to its own floor either way (only its own square:
    // the rest of its cell holds the margins other islands fill).
    let packed = |img| cell_texels(img, 4, 0.25);
    assert!(packed(&asked).len() > 4);
    assert_eq!(packed(&asked), packed(&floored));
    let raised = raised.expect("the packed island was raised");
    assert_eq!((raised.islands, raised.of), (1, 6));
    assert_eq!(raised.requested, between);
    assert_eq!(raised.low, raised.high);
    assert!((raised.low - small).abs() < 1e-4, "{raised:?}");
    let told = raised.to_string();
    assert!(
        told.contains(&format!("curvature_radius {} ", sig(between))) && told.contains("1 of 6"),
        "{told}"
    );
}

#[test]
fn a_radius_above_every_floor_is_used_as_asked_and_says_nothing() {
    let mesh = packed_cube();
    let r = cell_floor(0.25) * 1.5;
    let baked = Baked::new(&mesh, &settings(r)).unwrap().0;
    // The single radius the bake had before was `max(r, 3 × the mesh's average
    // texel)`: r here too, so this bake is the same, texel for texel.
    assert!(MIN_TEXELS * baked.texel_world() < r);
    let radii = Radii::new(&baked);
    assert!(radii.per_tri.iter().all(|&x| x == r), "{:?}", radii.per_tri);
    assert_eq!(radii.reach, r);
    assert_eq!(radii.raised, None);
    assert_eq!(curvature(&mesh, r).1, None);
}

#[test]
fn a_bake_naming_no_radius_reports_nothing_raised() {
    // At 4² every island's floor is past 2 % of the diagonal: wider, but not asked.
    let radii = Radii::new(&Baked::new(&packed_cube(), &Settings::new(4)).unwrap().0);
    assert!(
        radii.per_tri.iter().all(|&r| r > radii.reach),
        "{:?}",
        radii.per_tri
    );
    let raised = super::bake(
        &packed_cube(),
        &[MeshMap::Curvature],
        &Settings::new(4),
        |_, _| Ok(()),
    );
    assert_eq!(raised, Ok(None));
    // A bake without the curvature map draws no band, so it raises none.
    let ao = super::bake(&packed_cube(), &[MeshMap::Ao], &settings(0.01), |_, _| {
        Ok(())
    });
    assert_eq!(ao, Ok(None));
}

#[test]
fn an_island_packed_to_nearly_nothing_keeps_the_grid_at_the_reach() {
    let mesh = packed(1e-3);
    let r = cell_floor(1.0) * 1.5;
    let baked = Baked::new(&mesh, &settings(r)).unwrap().0;
    let (c, raised) = Curvature::new(&baked);
    assert_eq!(c.cell(), r);
    let raised = raised.expect("the packed island was raised");
    assert!(raised.high > 100.0 * r && raised.islands == 1, "{raised:?}");
}

#[test]
fn an_island_whose_uvs_enclose_no_area_is_not_raised() {
    let (_, raised) = curvature(&packed(0.0), cell_floor(1.0) * 1.5);
    assert_eq!(raised, None);
}

#[test]
fn the_warning_prints_three_figures_and_one_value_when_they_print_alike() {
    assert_eq!(
        [sig(0.0099996), sig(0.004), sig(0.0)],
        ["0.0100", "0.00400", "0"]
    );
    let told = Raised {
        requested: 0.004,
        low: 0.00531,
        high: 0.005312,
        islands: 2,
        of: 9,
    };
    let told = told.to_string();
    assert!(told.starts_with("curvature_radius 0.00400 is") && told.contains("at 0.00531;"));
}
