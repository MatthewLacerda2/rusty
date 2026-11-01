//! Light Probe Proxy Volume on the GPU (#1672): the issue's locker. A 2 m dynamic
//! box stands against a wall beside a `Baked` lamp at 2.2 m, lit only by baked
//! light probes (no ambient, black sky, and a `Baked` lamp is never drawn live).
//! With its one probe sample it is one colour top to bottom; with a volume its top,
//! by the lamp, is lit and its foot dark.
//!
//! Skips (passes without asserting) when no GPU / software adapter is present.

use glam::Vec3;
use rusty::components::{
    LightComponent, LightMode, LightProbeProxyVolumeComponent, LightType, ProxyResolutionMode,
};
use rusty::dev::bake::probe::bake_scene_probes;
use rusty::dev::screenshot::capture;
use rusty::scene::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

/// A box primitive at `position` scaled by `scale`; returns its id.
fn add(scene: &mut Scene, position: Vec3, scale: Vec3, is_static: bool) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene
        .world
        .set_mesh(id, primitive_mesh_component(Primitive::Box));
    scene.world.set_static(id, is_static);
    let mut t = scene.world.transform_mut(id).unwrap();
    (t.position, t.scale) = (position, scale);
    id
}

/// The floor, the wall behind the locker, the lamp, the locker (its id), and a
/// probe grid around it, baked.
fn baked_room() -> Option<(Scene, u32)> {
    let mut scene = Scene::new();
    crate::gpu::black_sky(&mut scene);
    scene.ambient_intensity = 0.0;
    add(
        &mut scene,
        Vec3::new(0.0, -0.05, 0.0),
        Vec3::new(6.0, 0.1, 6.0),
        true,
    );
    add(
        &mut scene,
        Vec3::new(0.0, 1.5, -0.7),
        Vec3::new(6.0, 3.0, 0.1),
        true,
    );
    let lamp = scene.add_entity("Lamp".to_string());
    let light = LightComponent {
        light_type: LightType::Point,
        mode: LightMode::Baked,
        intensity: 6.0,
        range: 3.0,
        ..rusty::scene::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.9, 2.2, -0.4);
    let locker = add(
        &mut scene,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.6, 2.0, 0.6),
        false,
    );
    let (min, max) = (Vec3::new(-0.8, 0.2, -0.45), Vec3::new(0.8, 2.2, 0.75));
    scene.probes.fill_grid(min, max, 0.6);
    bake_scene_probes(&mut scene)
        .unwrap()
        .then_some((scene, locker))
}

/// Mean red level of the locker's top and foot, seen from the lamp's side (+X):
/// the face whose top looks at the lamp.
fn top_and_foot(scene: &Scene, name: &str) -> (f64, f64) {
    let camera = Camera::new(Vec3::new(3.0, 1.0, 0.0), 180.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1672_proxy_{name}.png"));
    assert!(capture(scene, &camera, &path, SIZE, SIZE).expect("capture"));
    let img = image::open(&path).unwrap().to_rgb8();
    let level = |rows: std::ops::Range<u32>| {
        let px: Vec<f64> = rows
            .flat_map(|y| (29..35).map(move |x| (x, y)))
            .map(|(x, y)| f64::from(img.get_pixel(x, y).0[0]))
            .collect();
        px.iter().sum::<f64>() / px.len() as f64
    };
    (level(14..18), level(44..48))
}

#[test]
fn a_tall_locker_goes_from_a_lit_top_to_a_dark_foot_with_a_volume() {
    let Some((mut scene, locker)) = baked_room() else {
        eprintln!("[proxy volume] no GPU/software adapter — skipping");
        return;
    };
    let (top, foot) = top_and_foot(&scene, "single");
    let volume = LightProbeProxyVolumeComponent {
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [1, 8, 1],
        ..Default::default()
    };
    scene
        .world
        .set_light_probe_proxy_volume(locker, Some(volume));
    let (v_top, v_foot) = top_and_foot(&scene, "volume");
    eprintln!(
        "[proxy volume] single top={top:.1} foot={foot:.1}; volume top={v_top:.1} foot={v_foot:.1}"
    );
    assert!(
        (top - foot).abs() <= 3.0,
        "one sample is one colour: {top:.1} vs {foot:.1}"
    );
    assert!(
        v_top > v_foot + 15.0,
        "lit top, dark foot: {v_top:.1} vs {v_foot:.1}"
    );
    assert!(
        v_top > top && v_foot < foot,
        "the grid spreads the one sample apart"
    );
}
