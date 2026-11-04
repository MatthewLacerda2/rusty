//! The direction page's lamp share through BC7 (#1695): a lamp-free texel beside a
//! lamp's edge, hard or soft and inside a block, reads under the floor the shader
//! ignores, and the shader's floor and ramp are the ones the bake and this test use.

use glam::Vec4;

use super::encode::decode_direction;
use super::pages::{direction_levels, direction_page};
use crate::asset::lightmap_page::decode_bc7;

/// The most lamp share, in bytes of A, BC7 may lend a texel its bake stored none
/// in: the shader's `SHARE_FLOOR` (`forward/lightmap.wgsl`), which it treats as no
/// lamp and rescales the rest from. A sky-lit wall's page measured 4, a lamp's edge
/// inside a block 9, at every BC7 preset (#1695).
const SHARE_FLOOR: u8 = 16;

#[test]
fn bc7_keeps_a_lamp_free_texels_share_under_the_floor() {
    // A 32² page lit on the left, sky on the right, its direction turning texel by
    // texel as a real page's does. The lamp's edge sits at x = 14, inside a BC7
    // block at every level (14, 7, 3.5): hard on the top half, a shadow's or a
    // range's soft edge on the bottom (share 1 at x = 6 down to 0 at 14).
    let page: Vec<Vec4> = (0..1024u32)
        .map(|i| {
            let (x, y) = ((i % 32) as f32, (i / 32) as f32);
            let lamp = if y < 16.0 {
                1.0
            } else {
                ((14.0 - x) / 8.0).min(1.0)
            };
            match x < 14.0 {
                true => Vec4::new(-0.6, 0.6, 0.0, lamp),
                false => Vec4::new((x * 0.7).sin() * 0.4, 0.6, (y * 0.5).cos() * 0.4, 0.0),
            }
        })
        .collect();
    let encoded = direction_page(&page, 32, 1, &|| false).expect("never stopped");
    for level in 0..encoded.levels.len() {
        let want = direction_levels(&page, 32).swap_remove(level);
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
}

#[test]
fn the_shaders_floor_and_ramp_are_these() {
    let shader = include_str!("../../../../engine/shaders/forward/lightmap.wgsl");
    let floor = format!("const SHARE_FLOOR: f32 = {SHARE_FLOOR}.0 / 255.0;");
    assert!(shader.contains(&floor), "the shader's floor is this one");
    let ramp = format!(
        "const LAMP_RAMP: f32 = {:?};",
        super::bake::direction::LAMP_RAMP
    );
    assert!(shader.contains(&ramp), "the shader's ramp is the bake's");
}
