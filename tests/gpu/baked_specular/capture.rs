//! A bake capture never draws the lobe: a reflection probe sees a glossy, lightmapped
//! plane from one place for every viewer of the probe, so a highlight frozen into its
//! cube would be wrong. The probe baked over the plane reads the same, within a
//! rounding step, whether or not its lightmap is directional: the directions'
//! rebalance is 1 on a flat plane, which a GPU may round to a hair off it.

use glam::Vec3;
use rusty::components::LightMode;
use rusty::dev::bake::progress::BakeLog;
use rusty::dev::bake::reflection::{bake_scene_reflections, reflection_dir};

use super::{bake, scene};

/// One IEEE half float's value.
fn half(bits: u16) -> f32 {
    let (sign, exp, frac) = (
        bits >> 15,
        ((bits >> 10) & 0x1f) as i32,
        (bits & 0x3ff) as f32,
    );
    let magnitude = match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    };
    if sign == 1 {
        -magnitude
    } else {
        magnitude
    }
}

/// Every level's texels of the `Rgba16Float` cube a reflection bake wrote.
fn texels(bytes: &[u8]) -> Vec<f32> {
    let reader = ktx2::Reader::new(bytes).expect("a KTX2 cube");
    let halves = reader.levels().flat_map(|level| level.chunks_exact(2));
    halves
        .map(|b| half(u16::from_le_bytes([b[0], b[1]])))
        .collect()
}

#[test]
fn a_reflection_bake_never_sees_the_lobe() {
    let cube = |directional: bool| {
        let mut scene = scene(LightMode::Baked, 0.25);
        let probe = scene
            .reflection_probes
            .add(Vec3::new(-1.0, 0.5, 0.0), Vec3::splat(3.0));
        scene.reflection_probes.edit(probe, |p| p.manual = true);
        scene.lighting_settings.reflections.resolution = 16;
        let path = bake(&mut scene, &format!("probe_{directional}"), directional);
        let mut log = BakeLog::off();
        let ran = bake_scene_reflections(&mut scene, Some(&path), &mut log);
        ran.unwrap().then(|| {
            let dir = reflection_dir(&path);
            texels(&std::fs::read(dir.join("reflection_probe_0.ktx2")).expect("the baked cube"))
        })
    };
    let Some(lobed) = cube(true) else {
        return;
    };
    let plain = cube(false).unwrap();
    assert_eq!(lobed.len(), plain.len(), "the same cube");
    // A lobe would add the lamp's highlight, many times the plane's diffuse light; a
    // rounding difference is a part in a thousand of the texel.
    let worst = lobed
        .iter()
        .zip(&plain)
        .map(|(a, b)| (a - b).abs() / b.abs().max(1e-3))
        .fold(0.0, f32::max);
    eprintln!("[baked-spec] probe: at most {worst:.4} apart, relative");
    assert!(
        worst < 0.01,
        "the same cube with and without the lobe: {worst}"
    );
}
