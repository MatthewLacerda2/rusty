//! A bake capture never draws the lobe: a reflection probe sees a glossy, lightmapped
//! plane from one place for every viewer of the probe, so a highlight frozen into its
//! cube would be wrong. The probe baked over the plane reads the same, within a
//! rounding step, whether or not its lightmap is directional: the directions'
//! rebalance is 1 on a flat plane, which a GPU may round to a hair off it.

use glam::Vec3;
use rusty::asset::lightmap_page::bc6h_texels;
use rusty::components::LightMode;
use rusty::dev::bake::progress::BakeLog;
use rusty::dev::bake::reflection::{bake_scene_reflections, reflection_dir};

use super::{bake, scene};

/// Every level's channels of the BC6H cube a reflection bake wrote (#1885), decoded.
fn texels(bytes: &[u8]) -> Vec<f32> {
    let reader = ktx2::Reader::new(bytes).expect("a KTX2 cube");
    let size = reader.header().pixel_width;
    let levels = reader.levels().enumerate();
    let faces = levels.flat_map(|(l, level)| {
        let edge = (size >> l).max(1);
        level.chunks_exact(level.len() / 6).map(move |f| (edge, f))
    });
    faces
        .flat_map(|(edge, face)| bc6h_texels(edge, face))
        .flat_map(|t| t.to_array())
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
        let path = bake(
            &mut scene,
            &format!("probe_{directional}"),
            directional,
            super::DENSITY,
        );
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
    // A NaN or an infinity would vanish in the fold below (`inf - inf` is NaN).
    assert!(
        lobed.iter().chain(&plain).all(|t| t.is_finite()),
        "every texel is finite"
    );
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
