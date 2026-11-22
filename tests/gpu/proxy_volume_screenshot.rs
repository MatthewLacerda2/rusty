//! Light Probe Proxy Volume on the GPU: a 2 m dynamic box beside a `Baked` lamp,
//! lit only by baked probes. One probe sample gives one colour top to bottom; a volume
//! lights its top and leaves its foot dark; a `Custom` box over the top quarter makes
//! the foot read the box's face (`proxy_volume_clamp`). Skips without an adapter.

use glam::Vec3;
use rusty::components::{
    LightComponent, LightMode, LightProbeProxyVolumeComponent, LightType, ProxyResolutionMode,
};
use rusty::dev::bake::probe::bake_scene_probes;
use rusty::dev::bake::progress::BakeLog;
use rusty::dev::{capture::CaptureHost, screenshot::capture_into};
use rusty::edit::authoring::{primitive_mesh_component, Primitive};
use rusty::scene::{Camera, Scene};

const SIZE: u32 = 64;

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

pub(super) fn baked_room() -> Option<(Scene, u32)> {
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
        ..rusty::edit::authoring::default_light()
    };
    scene.world.set_light(lamp, Some(light));
    scene.lightmaps.took_in_baked(lamp);
    scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.9, 2.2, -0.4);
    let locker = add(
        &mut scene,
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.6, 2.0, 0.6),
        false,
    );
    let (min, max) = (Vec3::new(-0.8, 0.2, -0.45), Vec3::new(0.8, 2.2, 0.75));
    scene.probes.fill_grid(min, max, 0.6);
    bake_scene_probes(&mut scene, &mut BakeLog::off())
        .unwrap()
        .then_some((scene, locker))
}

/// Mean red of the locker's top and foot from +X; `host` is shared so the shots
/// reuse one renderer (one each cost a third of the time on Metal).
pub(super) fn top_and_foot(host: &mut CaptureHost, scene: &Scene, name: &str) -> (f64, f64) {
    let camera = Camera::new(Vec3::new(3.0, 1.0, 0.0), 180.0, 0.0);
    let path = crate::temp::dir().join(format!("rusty_gpu_1672_proxy_{name}.png"));
    assert!(capture_into(host, scene, &camera, &path, SIZE, SIZE).expect("capture"));
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
fn a_tall_locker_goes_from_a_lit_top_to_a_dark_foot_with_a_volume_and_clamps_to_a_box() {
    let Some((mut scene, locker)) = baked_room() else {
        eprintln!("[proxy volume] no GPU/software adapter — skipping");
        return;
    };
    let mut host = CaptureHost::new();
    let (top, foot) = top_and_foot(&mut host, &scene, "single");
    let volume = LightProbeProxyVolumeComponent {
        resolution_mode: ProxyResolutionMode::Custom,
        grid_resolution: [1, 8, 1],
        ..Default::default()
    };
    scene
        .world
        .set_light_probe_proxy_volume(locker, Some(volume));
    let (v_top, v_foot) = top_and_foot(&mut host, &scene, "volume");
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
    super::proxy_volume_clamp::a_foot_outside_a_custom_box_reads_the_box_s_face_not_the_single_sample(
        &mut host, &mut scene, locker, foot,
    );
}
