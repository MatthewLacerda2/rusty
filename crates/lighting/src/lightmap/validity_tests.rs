//! Texels inside geometry (#1949): filled from their own chart, never a neighbour's,
//! and judged the same whatever the sample count.

use glam::{Vec2, Vec3};

use super::tests::sun;
use super::*;
use rusty_scene::lighting::settings::{BakeSettings, Denoiser};

/// A sunlit 4×4 floor (y = 0, facing up) and a 4×4 underside (y = -1, facing
/// down, which nothing lights) as two charts of one mesh, side by side in its
/// lightmap UV with a texel's gap between them: the floor on `u` 0.05–0.48, the
/// underside on 0.52–0.95. The floor's `+X` edge (`x = 2`) is the chart edge that
/// faces the dark one.
fn two_charts() -> BakeMesh {
    let floor = [(-2.0, 2.0), (2.0, 2.0), (2.0, -2.0), (-2.0, -2.0)];
    let under = [(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)];
    let mut positions: Vec<Vec3> = floor.map(|(x, z)| Vec3::new(x, 0.0, z)).to_vec();
    positions.extend(under.map(|(x, z)| Vec3::new(x, -1.0, z)));
    let mut normals = vec![Vec3::Y; 4];
    normals.extend([Vec3::NEG_Y; 4]);
    let uv = |u0: f32, u1: f32, flip: bool| {
        let (a, b) = if flip { (u1, u0) } else { (u0, u1) };
        [(a, 0.95), (b, 0.95), (b, 0.05), (a, 0.05)].map(|(u, v)| Vec2::new(u, v))
    };
    let mut lightmap_uvs = uv(0.05, 0.48, false).to_vec();
    lightmap_uvs.extend(uv(0.52, 0.95, true));
    BakeMesh {
        entity: 1,
        positions,
        normals,
        lightmap_uvs,
        indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
        runs: vec![run(12)],
        ..Default::default()
    }
}

fn run(end: u32) -> BakeRun {
    let (albedo, emissive) = (Vec3::splat(0.5), Vec3::ZERO);
    BakeRun {
        end,
        albedo,
        emissive,
        occludes: true,
    }
}

/// A closed box `min..max`, faces outward, receiving no lightmap.
fn crate_box(min: Vec3, max: Vec3) -> BakeMesh {
    let corner = |i: u32| Vec3::select(glam::BVec3::new(i & 1 > 0, i & 2 > 0, i & 4 > 0), max, min);
    let positions: Vec<Vec3> = (0..8).map(corner).collect();
    #[rustfmt::skip]
    let quads = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
    let indices = quads
        .iter()
        .flat_map(|q| [q[0], q[1], q[2], q[0], q[2], q[3]])
        .collect();
    BakeMesh {
        entity: 2,
        normals: vec![Vec3::Y; 8],
        lightmap_uvs: vec![Vec2::ZERO; 8],
        positions,
        indices,
        runs: vec![run(36)],
        ..Default::default()
    }
}

/// The floor, the dark underside and a crate from `x = wall` over the floor's `+X` edge,
/// so the floor texels under it are inside geometry, lit by a baked sun from above.
fn room(wall: f32) -> BakeScene {
    let mut light = sun(true);
    light.shape = LightShape::Directional {
        direction: Vec3::NEG_Y,
    };
    let crate_ = crate_box(Vec3::new(wall, -0.5, -1.0), Vec3::new(3.0, 1.0, 1.0));
    BakeScene {
        shadowmask: Default::default(),
        meshes: vec![two_charts(), crate_],
        lights: vec![light],
        sky: Vec3::ZERO.into(),
        diffuse: Default::default(),
    }
}

fn settings() -> BakeSettings {
    BakeSettings {
        texels_per_unit: 4.0,
        bounces: 1,
        denoiser: Denoiser::None,
        filter_radius: 0,
        ..Default::default()
    }
}

/// The floor chart's texels, one texel in from its UV edges.
fn floor_texels(map: &Lightmap) -> Vec<Vec3> {
    let size = map.size as f32;
    let inside = |c: f32, lo: f32, hi: f32| c > lo * size + 1.0 && c < hi * size - 1.0;
    let floor = |i: &usize| {
        let (x, y) = (*i as u32 % map.size, *i as u32 / map.size);
        inside(x as f32 + 0.5, 0.05, 0.48) && inside(y as f32 + 0.5, 0.05, 0.95)
    };
    (0..map.texels.len())
        .filter(floor)
        .map(|i| map.texels[i])
        .collect()
}

#[test]
fn an_invalid_texel_is_filled_from_its_own_chart() {
    let maps = bake(&room(-0.5), &settings());
    let map = maps.iter().find(|m| m.entity == 1).expect("baked");
    // The sun lights the open floor head-on: E = 3, stored as E / π.
    let lit = 3.0 / std::f32::consts::PI;
    let floor = floor_texels(map);
    assert!(floor.len() > 50, "the floor chart has texels");
    let darkest = floor.iter().map(|t| t.x).fold(f32::MAX, f32::min);
    assert!(
        darkest > lit * 0.9,
        "a floor texel under the crate took the dark chart's light: {darkest} vs {lit}"
    );
}

#[test]
fn validity_does_not_depend_on_the_sample_count() {
    // Stand the crate's -X wall on the centre of a floor texel, so it sees half back
    // faces. Its light is the sun's (unsampled) or what it was filled with, so the
    // map only changes with the sample count (beyond a little bounce) if its
    // validity does: it used to.
    let size = lightmap_size(&two_charts(), settings().texels_per_unit, 1024).expect("UVs");
    let u = 5.5 / size as f32;
    let wall = -2.0 + (u - 0.05) / 0.43 * 4.0;
    let at = |samples| {
        let maps = bake(
            &room(wall),
            &BakeSettings {
                samples,
                ..settings()
            },
        );
        maps.into_iter().find(|m| m.entity == 1).expect("baked")
    };
    let (few, many) = (floor_texels(&at(2)), floor_texels(&at(64)));
    let moved = few
        .iter()
        .zip(&many)
        .filter(|(a, b)| (a.x - b.x).abs() > 0.2);
    assert_eq!(moved.count(), 0, "texels changed with the sample count");
}
