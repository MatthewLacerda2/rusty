use glam::{Vec2, Vec3};

use super::fixtures::{bake_with, cell, cube, l_shape, quad};
use super::{curvature, rays, Baked, MeshMap, MeshSource, Settings};
use rusty_procgen::image_buf::{Image, Size};

fn one(mesh: &MeshSource, map: MeshMap, settings: Settings) -> Image {
    bake_with(mesh, &[map], &settings).unwrap().remove(0).1
}

fn red(img: &Image, x: u32, y: u32) -> f32 {
    img.pixels()[(y * img.width() + x) as usize][0]
}

#[test]
fn the_defaults_are_the_constants_the_bake_had_before_they_were_settings() {
    let s = Settings::new(1024);
    assert_eq!(s.size, Size::new(1024, 1024));
    assert_eq!((s.ao_rays, s.thickness_rays), (64, 64));
    assert_eq!(rays::SAMPLES, 64);
    assert_eq!(s.curvature_radius, None);
    assert_eq!(s.curvature_reach(2.0), curvature::RADIUS * 2.0);
    assert_eq!(curvature::RADIUS, 0.02);
}

fn door() -> MeshSource {
    let mut m = cube();
    for p in &mut m.positions {
        p.z *= 0.012;
    }
    m
}

fn band(settings: Settings) -> u32 {
    let size = settings.size.width;
    let img = one(&door(), MeshMap::Curvature, settings);
    let (lo, hi) = cell(4);
    let y = (((lo.y + hi.y) * 0.5) * size as f32) as u32;
    let x0 = (lo.x * size as f32).ceil() as u32;
    (x0..(hi.x * size as f32) as u32)
        .take_while(|&x| red(&img, x, y) > 0.5 + 1e-3)
        .count() as u32
}

#[test]
fn a_curvature_radius_narrows_the_band_on_a_12_mm_door() {
    let default = band(Settings::new(1024));
    let narrow = band(Settings {
        curvature_radius: Some(0.003),
        ..Settings::new(1024)
    });
    assert!(default >= 5, "the default band is {default} texels");
    assert!(narrow <= 1, "a 3 mm band is {narrow} texels");
}

fn floor_ao(rays: u32) -> Vec<f32> {
    const S: u32 = 32;
    let settings = Settings {
        ao_rays: rays,
        ..Settings::new(S)
    };
    let img = one(&l_shape(), MeshMap::Ao, settings);
    let (lo, hi) = cell(0);
    let (lo, hi) = ((lo * S as f32).ceil(), (hi * S as f32).floor());
    (lo.y as u32..hi.y as u32)
        .flat_map(|y| (lo.x as u32..hi.x as u32).map(move |x| (x, y)))
        .map(|(x, y)| red(&img, x, y))
        .collect()
}

fn grain(rays: u32, reference: &[f32]) -> f32 {
    let ao = floor_ao(rays);
    let off: f32 = ao.iter().zip(reference).map(|(a, r)| (a - r).abs()).sum();
    off / ao.len() as f32
}

#[test]
fn more_ao_rays_lower_the_grain() {
    let reference = floor_ao(1024);
    let (coarse, fine) = (grain(16, &reference), grain(256, &reference));
    assert!(fine < coarse * 0.5, "16 rays {coarse}, 256 rays {fine}");
}

#[test]
fn a_non_square_size_stretches_uv0_over_it() {
    let wide = one(&cube(), MeshMap::Position, Settings::new(Size::new(64, 16)));
    let square = one(&cube(), MeshMap::Position, Settings::new(64));
    assert_eq!((wide.width(), wide.height()), (64, 16));
    let mut checked = 0;
    for k in 0..6 {
        let (lo, hi) = cell(k);
        let inside = |uv: Vec2, pad: Vec2| uv.cmpge(lo + pad).all() && uv.cmple(hi - pad).all();
        for y in 0..16 {
            for x in 0..64 {
                let uv = Vec2::new((x as f32 + 0.5) / 64.0, (y as f32 + 0.5) / 16.0);
                if !inside(uv, Vec2::new(2.0 / 64.0, 2.0 / 16.0)) {
                    continue;
                }
                let at = |img: &Image, x: u32, y: u32| {
                    Vec3::from_slice(&img.pixels()[(y * img.width() + x) as usize][..3])
                };
                let mean = (at(&square, x, 4 * y + 1) + at(&square, x, 4 * y + 2)) * 0.5;
                let got = at(&wide, x, y);
                assert!(
                    got.distance(mean) < 1e-4,
                    "face {k} texel ({x}, {y}): {got} vs {mean}"
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 50, "only {checked} interior texels compared");
}

#[test]
fn a_setting_out_of_range_is_refused_naming_it() {
    let err = |s: Settings| bake_with(&cube(), &[MeshMap::Ao], &s).unwrap_err();
    let base = Settings::new(8);
    assert!(err(Settings {
        size: Size::new(8, 0),
        ..base.clone()
    })
    .contains("size {8, 0}"));
    assert!(err(Settings {
        ao_rays: 0,
        ..base.clone()
    })
    .contains("ao_rays 0"));
    assert!(err(Settings {
        thickness_rays: 5000,
        ..base.clone()
    })
    .contains("thickness_rays 5000"));
    for r in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let e = err(Settings {
            curvature_radius: Some(r),
            ..base.clone()
        });
        assert!(e.contains("curvature_radius"), "{e}");
    }
    let wide = |r| Settings {
        curvature_radius: Some(r),
        ..base.clone()
    };
    let e = bake_with(&cube(), &[MeshMap::Curvature], &wide(2.0)).unwrap_err();
    assert!(
        e.contains("curvature_radius 2 is above") && e.contains("1.73"),
        "{e}"
    );
    assert!(bake_with(&cube(), &[MeshMap::Curvature], &wide(1.7)).is_ok());
}

fn sheet(w: f32, h: f32, normal: Vec3) -> MeshSource {
    let mut m = MeshSource::default();
    let c = [Vec3::ZERO, Vec3::X * w, Vec3::new(w, h, 0.0), Vec3::Y * h];
    quad(&mut m, c, (Vec2::ZERO, Vec2::ONE), 0);
    m.normals.fill(normal.normalize());
    m
}

#[test]
fn a_texel_is_measured_by_its_area_so_a_strip_floors_at_three_real_texels() {
    let strip = sheet(64.0, 1.0, Vec3::Z);
    let settings = Settings::new(Size::new(4096, 64));
    let baked = Baked::new(&strip, &settings).unwrap().0;
    assert!(
        (baked.texel_world() * 64.0 - 1.0).abs() < 1e-5,
        "{}",
        baked.texel_world()
    );
    let cube = cube();
    let settings = Settings::new(48);
    let baked = Baked::new(&cube, &settings).unwrap().0;
    let per_uv = crate::lightmap::world_per_uv(&cube.surface()).unwrap();
    assert_eq!(baked.texel_world(), per_uv / 48.0);
}

#[test]
fn a_texel_whose_every_ray_is_dropped_reads_the_neutral_value() {
    let leaning = sheet(1.0, 1.0, Vec3::new(1.0, 0.0, 0.05));
    let settings = Settings {
        ao_rays: 1,
        thickness_rays: 1,
        ..Settings::new(16)
    };
    for map in [MeshMap::Ao, MeshMap::Thickness] {
        let img = one(&leaning, map, settings.clone());
        let low = img.pixels().iter().filter(|p| p[0] < 1.0).count();
        assert_eq!(low, 0, "{map:?}: {low} texels below 1");
    }
}

#[test]
fn ao_distance_bounds_ao_alone() {
    let l = l_shape();
    let thickness = |ao_distance| {
        let s = Settings {
            ao_distance,
            ..Settings::new(16)
        };
        bake_with(&l, &[MeshMap::Thickness], &s)
            .unwrap()
            .remove(0)
            .1
    };
    assert_eq!(thickness(None).pixels(), thickness(Some(0.01)).pixels());
}

#[test]
fn an_ao_distance_that_is_no_distance_is_refused_naming_it() {
    for d in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        let s = Settings {
            ao_distance: Some(d),
            ..Settings::new(8)
        };
        let e = bake_with(&cube(), &[MeshMap::Ao], &s).unwrap_err();
        assert!(
            e.contains("ao_distance") && e.contains("positive distance"),
            "{e}"
        );
    }
}
