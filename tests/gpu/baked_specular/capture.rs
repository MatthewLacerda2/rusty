//! A bake capture never draws the lobe: a reflection probe sees a glossy, lightmapped
//! plane from one place for every viewer of the probe, so a highlight frozen into its
//! cube would be wrong. The probe baked over the plane is the same bytes whether or not
//! its lightmap is directional (the directions' rebalance is exactly 1 on a flat plane).

use glam::Vec3;
use rusty::components::LightMode;
use rusty::dev::bake::progress::BakeLog;
use rusty::dev::bake::reflection::{bake_scene_reflections, reflection_dir};

use super::{bake, scene};

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
            std::fs::read(dir.join("reflection_probe_0.ktx2")).expect("the baked cube")
        })
    };
    let Some(lobed) = cube(true) else {
        return;
    };
    assert!(
        lobed == cube(false).unwrap(),
        "the same bytes with and without the lobe"
    );
}
