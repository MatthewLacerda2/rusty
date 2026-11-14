//! A surface that adds onto or multiplies what is behind it takes no light from
//! the air in front of it (#1703): what is behind already carries that air. In
//! fog, a black Additive card and a white (emissive, so lit white) Multiply sheet
//! stay invisible, where adding the air's in-scatter would draw a glowing
//! rectangle, brighter the thicker the fog.

use glam::Vec3;

use super::fixture::{fog, glow, lamp, renderer, room, settled, tinted, LAMP_AT};
use rusty_components::{BlendMode, LightMode, MaterialAsset, RenderMode, VolumetricFog};
use rusty_scene::Scene;

/// Where the sheet hangs: in the lamp-lit air, left of the lamp, facing the eye.
const SHEET: Vec3 = Vec3::new(-1.0, 1.5, -3.0);

/// The room with its lamp under `volumetric`, and with a sheet of `mode` when
/// `sheet` holds its base colour and emission.
fn scene(volumetric: VolumetricFog, sheet: Option<(BlendMode, [f32; 3], [f32; 3])>) -> Scene {
    let mut scene = room(volumetric);
    lamp(&mut scene, LAMP_AT, 3.0, LightMode::Realtime);
    if let Some((mode, base, emissive)) = sheet {
        let at = (SHEET, Vec3::new(1.0, 1.0, 0.02));
        tinted(&mut scene, "sheet", at, base);
        scene.materials.insert(
            "sheet".into(),
            MaterialAsset {
                base_color: base,
                emissive,
                roughness: 1.0,
                render_mode: RenderMode::Transparent,
                blend_mode: mode,
                alpha: 1.0,
                ..MaterialAsset::default()
            },
        );
    }
    scene
}

/// What the sheet changes where it hangs, in clear air and in fog: the glow
/// with it less the glow without it.
fn changes(mode: BlendMode, base: [f32; 3], emissive: [f32; 3]) -> Option<(f32, f32, f32)> {
    let mut r = renderer()?;
    let sheet = Some((mode, base, emissive));
    let mut read = |v: VolumetricFog, s| glow(&settled(&mut r, &scene(v, s), 4), SHEET);
    let clear = read(VolumetricFog::default(), sheet) - read(VolumetricFog::default(), None);
    let air = read(fog(), None);
    Some((clear, read(fog(), sheet) - air, air))
}

#[test]
fn gpu_a_black_additive_card_is_invisible_in_fog() {
    let Some((clear, fogged, air)) = changes(BlendMode::Additive, [0.0; 3], [0.0; 3]) else {
        return;
    };
    eprintln!("[volumetric] additive card adds {clear} clear, {fogged} in fog (air {air})");
    assert!(air > 0.5, "the air there is lit: {air}");
    // Its own faint highlight, dimmed: never the air's light again.
    assert!(
        fogged <= clear.max(0.0) + 0.1,
        "{fogged} in fog vs {clear} clear"
    );
}

#[test]
fn gpu_a_white_multiply_sheet_is_invisible_in_fog() {
    let Some((clear, fogged, air)) = changes(BlendMode::Multiply, [0.0; 3], [1.0; 3]) else {
        return;
    };
    eprintln!("[volumetric] multiply sheet changes {clear} clear, {fogged} in fog (air {air})");
    assert!(air > 0.5, "the air there is lit: {air}");
    assert!(
        fogged.abs() < 0.05 * air + 0.1,
        "{fogged} in fog over {air}"
    );
}
