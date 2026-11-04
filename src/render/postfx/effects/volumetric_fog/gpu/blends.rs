//! A surface that adds onto or multiplies what is behind it takes no light from
//! the air in front of it (#1703): what is behind already carries that air. In
//! fog, a black Additive card and a white (emissive, so lit white) Multiply sheet
//! stay invisible, where adding the air's in-scatter would draw a glowing
//! rectangle, brighter the thicker the fog.

use glam::Vec3;

use super::fixture::{fog, glow, lamp, renderer, room, settled, tinted, LAMP_AT};
use crate::components::{BlendMode, LightMode, MaterialAsset, RenderMode};
use crate::scene::Scene;

/// Where the sheet hangs: in the lamp-lit air, left of the lamp, facing the eye.
const SHEET: Vec3 = Vec3::new(-1.0, 1.5, -3.0);

/// The fogged room with its lamp, and with a sheet of `mode` when `sheet` holds
/// its base colour and emission.
fn scene(sheet: Option<(BlendMode, [f32; 3], [f32; 3])>) -> Scene {
    let mut scene = room(fog());
    lamp(&mut scene, LAMP_AT, 3.0, LightMode::Realtime);
    if let Some((mode, base, emissive)) = sheet {
        let at = (SHEET, Vec3::new(1.0, 1.0, 0.02));
        tinted(&mut scene, "sheet", at, base);
        scene.materials.insert(
            "sheet".into(),
            MaterialAsset {
                base_color: base,
                emissive,
                render_mode: RenderMode::Transparent,
                blend_mode: mode,
                alpha: 1.0,
                ..MaterialAsset::default()
            },
        );
    }
    scene
}

/// The glow where the sheet hangs, without it and with it.
fn without_and_with(mode: BlendMode, base: [f32; 3], emissive: [f32; 3]) -> Option<(f32, f32)> {
    let mut r = renderer()?;
    let without = glow(&settled(&mut r, &scene(None), 4), SHEET);
    let with = glow(
        &settled(&mut r, &scene(Some((mode, base, emissive))), 4),
        SHEET,
    );
    Some((without, with))
}

#[test]
fn gpu_a_black_additive_card_is_invisible_in_fog() {
    let Some((without, with)) = without_and_with(BlendMode::Additive, [0.0; 3], [0.0; 3]) else {
        return;
    };
    eprintln!("[volumetric] additive card: {without} without, {with} with");
    assert!(without > 0.5, "the air there is lit: {without}");
    assert!(
        (with - without).abs() < 0.05 * without + 0.1,
        "{with} vs {without}"
    );
}

#[test]
fn gpu_a_white_multiply_sheet_is_invisible_in_fog() {
    let Some((without, with)) = without_and_with(BlendMode::Multiply, [0.0; 3], [1.0; 3]) else {
        return;
    };
    eprintln!("[volumetric] multiply sheet: {without} without, {with} with");
    assert!(without > 0.5, "the air there is lit: {without}");
    assert!(
        (with - without).abs() < 0.05 * without + 0.1,
        "{with} vs {without}"
    );
}
