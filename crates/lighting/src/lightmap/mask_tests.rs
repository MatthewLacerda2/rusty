//! A `Shadowmask` bake's masks (#1695): per channel, a `Mixed` light's static
//! occlusion, in the lightmaps and at the probes, leaving the lightmap's own light
//! as a `BakedIndirect` bake has it.

use glam::{Vec3, Vec4};

use super::bake::bake_on;
use super::tests::{floor, quad, settings};
use super::*;

/// The floor and, 1 unit over its `x < 0` half, a panel facing down onto it.
fn half_covered(lights: Vec<BakeLight>) -> BakeScene {
    let p = |x, z| Vec3::new(x, 1.0, z);
    let panel = quad(
        3,
        [p(-2.0, -2.0), p(0.0, -2.0), p(0.0, 2.0), p(-2.0, 2.0)],
        Vec3::splat(0.5),
    );
    BakeScene {
        meshes: vec![floor(), panel],
        lights,
        ..Default::default()
    }
}

/// A `Mixed` point light 3 units over the floor's centre, on `channel`.
fn lamp(channel: Option<u8>) -> BakeLight {
    BakeLight {
        shape: LightShape::Point {
            position: Vec3::Y * 3.0,
            range: 10.0,
        },
        radiance: Vec3::splat(4.0),
        bakes_direct: false,
        mask_channel: channel,
    }
}

/// `scene` masking channel 0, as `BakeScene::gather` marks a masking bake.
fn masking(mut scene: BakeScene) -> BakeScene {
    scene.shadowmask.channels.insert(9, 0);
    scene
}

fn floor_map(maps: &[Lightmap]) -> &Lightmap {
    maps.iter()
        .find(|m| m.entity == 1)
        .expect("the floor is baked")
}

#[test]
fn a_mixed_lights_mask_is_its_static_shadow_and_the_light_stays_out() {
    let s = settings();
    let maps = bake(&masking(half_covered(vec![lamp(Some(0))])), &s);
    let masks = &floor_map(&maps).masks;
    assert_eq!(masks.len(), floor_map(&maps).texels.len());
    let shadowed = masks.iter().filter(|m| m.x == 0.0).count() as f32;
    let share = shadowed / masks.len() as f32;
    // The panel's shadow from 3 m up covers the floor's x < 0 half.
    assert!((0.35..=0.65).contains(&share), "{share}");
    assert!(
        masks.iter().all(|m| m.x == 0.0 || m.x == 1.0),
        "one ray each"
    );
    assert!(
        masks.iter().all(|m| m.y == 1.0 && m.z == 1.0 && m.w == 1.0),
        "unused channels lit"
    );
    // The lightmap holds what it held without a mask: the bounce, no direct light.
    let unmasked = bake(&half_covered(vec![lamp(None)]), &s);
    assert_eq!(floor_map(&maps).texels, floor_map(&unmasked).texels);
    assert!(floor_map(&unmasked).masks.is_empty(), "no mask without one");
}

#[test]
fn masks_bake_the_same_on_any_worker_count_and_without_directions() {
    let scene = masking(half_covered(vec![lamp(Some(2))]));
    let s = settings();
    let progress = BakeProgress::default();
    let one = bake_on(&scene, &s, &progress, 1).unwrap();
    let many = bake_on(&scene, &s, &progress, 5).unwrap();
    assert_eq!(one, many, "the thread count never shows");
    let flat = BakeSettings {
        directional: false,
        ..s
    };
    let maps = bake(&scene, &flat);
    assert!(floor_map(&maps).directions.is_empty());
    assert_eq!(floor_map(&maps).masks, floor_map(&one).masks);
}

#[test]
fn the_atlas_packs_masks_beside_the_pages_on_a_lit_background() {
    let maps = bake(&masking(half_covered(vec![lamp(Some(0))])), &settings());
    let atlas = pack(&maps, 64);
    assert_eq!(atlas.masks.len(), atlas.pages.len());
    let background = atlas.masks[0].iter().filter(|&&m| m == Vec4::ONE).count();
    assert!(background > 0, "lit where no chart is");
    let no_masks: Vec<Lightmap> = maps
        .into_iter()
        .map(|m| Lightmap {
            masks: Vec::new(),
            ..m
        })
        .collect();
    assert!(pack(&no_masks, 64).masks.is_empty());
}

#[test]
fn a_probe_reads_the_same_occlusion_and_bad_input_reads_lit() {
    let scene = half_covered(vec![lamp(Some(1))]);
    let under = Vec3::new(-1.0, 0.5, 0.0);
    let open = Vec3::new(1.0, 0.5, 0.0);
    let got = mixed_occlusion(&scene, &[under, open, Vec3::NAN]);
    assert_eq!(got, [Vec4::new(1.0, 0.0, 1.0, 1.0), Vec4::ONE, Vec4::ONE]);
    assert!(mixed_occlusion(&scene, &[]).is_empty());
    let unmasked = half_covered(vec![lamp(None)]);
    assert_eq!(mixed_occlusion(&unmasked, &[under]), [Vec4::ONE]);
}

#[test]
fn a_mask_byte_round_trips_and_clamps() {
    for m in [Vec4::ONE, Vec4::ZERO, Vec4::new(0.0, 1.0, 0.5, 0.25)] {
        assert!(
            (decode_mask(encode_masks(&[m]).try_into().unwrap()) - m)
                .abs()
                .max_element()
                < 0.003
        );
    }
    let wild = encode_masks(&[Vec4::new(2.0, -1.0, f32::NAN, 1.0)]);
    assert_eq!(wild[..2], [255, 0]);
}
