//! The direction page's lamp share (#1695): only `Baked` lamps' direct light makes a
//! texel's specular lobe. An even sky over a dark ground and a `Mixed` light's bounce
//! leave a texel directional but lamp-free; a lamp alone gives a share of 1 toward
//! itself; two opposed lamps weaken it; and BC7 keeps a lamp-free texel's share
//! under the floor the shader ignores.

use glam::{Vec3, Vec4};

use super::tests::{floor, quad, settings, sun, wall};
use super::*;
use crate::asset::lightmap_page::decode_bc7;

/// The most lamp share, in bytes of A, BC7 may lend a texel its bake stored none
/// in: the shader's `SHARE_FLOOR` (`forward/lightmap.wgsl`), which it treats as no
/// lamp and rescales the rest from. A sky-lit wall's page measured 4 (#1695).
const SHARE_FLOOR: u8 = 8;

/// Every direction texel of `entity`'s lightmap, and their mean.
fn texels(maps: &[Lightmap], entity: u32) -> (Vec<Vec4>, Vec4) {
    let map = maps.iter().find(|m| m.entity == entity).expect("baked");
    let mean = map.directions.iter().sum::<Vec4>() / map.directions.len() as f32;
    (map.directions.clone(), mean)
}

fn scene(meshes: Vec<BakeMesh>, lights: Vec<BakeLight>, sky: f32) -> BakeScene {
    BakeScene {
        meshes,
        lights,
        sky: Vec3::splat(sky).into(),
        diffuse: Default::default(),
    }
}

#[test]
fn a_sky_over_dark_ground_gives_direction_but_no_lamp_share() {
    let p = |x, z| Vec3::new(x, 0.0, z);
    let ground = quad(
        1,
        [p(-8.0, 8.0), p(8.0, 8.0), p(8.0, -8.0), p(-8.0, -8.0)],
        Vec3::splat(0.02),
    );
    let maps = bake(&scene(vec![ground, wall()], Vec::new(), 1.0), &settings());
    let (all, mean) = texels(&maps, 2);
    assert!(
        mean.truncate().length() > 0.4,
        "the sky alone aims it: {mean}"
    );
    assert!(all.iter().all(|d| d.w == 0.0), "no lamp: share 0");
}

#[test]
fn a_mixed_lights_bounce_has_no_lamp_share() {
    // The sun grazes the floor and lights it only off the wall (`tests`).
    let maps = bake(
        &scene(vec![floor(), wall()], vec![sun(false)], 0.0),
        &settings(),
    );
    let (all, mean) = texels(&maps, 1);
    assert!(
        mean.truncate().length() > 0.3,
        "the bounce has a direction: {mean}"
    );
    assert!(
        all.iter().all(|d| d.w == 0.0),
        "a Mixed light's bounce is not a lamp"
    );
}

#[test]
fn a_baked_lamp_alone_has_its_whole_share_toward_itself() {
    let maps = bake(
        &scene(vec![floor(), wall()], vec![sun(true)], 0.0),
        &settings(),
    );
    let (_, mean) = texels(&maps, 2);
    assert!(
        mean.w > 0.95,
        "nearly all the wall's light is the sun's: {mean}"
    );
    assert!(
        mean.truncate().normalize().dot(-Vec3::X) > 0.99,
        "toward it: {mean}"
    );
}

#[test]
fn two_opposed_lamps_weaken_the_share() {
    let lamp = |x: f32| BakeLight {
        shape: LightShape::Directional {
            direction: Vec3::new(x, -1.0, 0.0).normalize(),
        },
        ..sun(true)
    };
    let maps = bake(
        &scene(vec![floor()], vec![lamp(1.0), lamp(-1.0)], 0.0),
        &settings(),
    );
    let (_, mean) = texels(&maps, 1);
    // Each arrives 45° off the normal: their sideways halves cancel.
    let half = std::f32::consts::FRAC_1_SQRT_2;
    assert!((mean.w - half).abs() < 0.02, "cos 45°: {mean}");
    assert!(
        mean.truncate().normalize().dot(Vec3::Y) > 0.99,
        "between them: {mean}"
    );
}

#[test]
fn bc7_keeps_a_lamp_free_texels_share_under_the_floor() {
    // A 32² page: the left half under a lamp (share 1), the right half under sky,
    // its direction turning texel by texel as a real page's does.
    let page: Vec<Vec4> = (0..1024u32)
        .map(|i| {
            let (x, y) = ((i % 32) as f32, (i / 32) as f32);
            match x < 16.0 {
                true => Vec4::new(-0.6, 0.6, 0.0, 1.0),
                false => Vec4::new((x * 0.7).sin() * 0.4, 0.6, (y * 0.5).cos() * 0.4, 0.0),
            }
        })
        .collect();
    let encoded = direction_page(&page, 32, 1, &|| false).expect("never stopped");
    for level in 0..encoded.levels.len() {
        let want = super::pages::direction_levels(&page, 32).swap_remove(level);
        for (i, (w, got)) in want.iter().zip(decode_bc7(&encoded, level)).enumerate() {
            if w[3] == 0 {
                assert!(got[3] <= SHARE_FLOOR, "level {level} texel {i}: {got:?}");
            }
        }
    }
    let back = decode_direction(decode_bc7(&encoded, 0)[0]);
    assert!(
        (back - page[0]).length() < 0.03,
        "a lamp texel survives: {back}"
    );
    let shader = include_str!("../../../../engine/shaders/forward/lightmap.wgsl");
    let mirror = format!("const SHARE_FLOOR: f32 = {SHARE_FLOOR}.0 / 255.0;");
    assert!(shader.contains(&mirror), "the shader's floor is this one");
}
