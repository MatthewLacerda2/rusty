//! The probe bake honours light modes (#809), end to end on the GPU: a `Realtime`
//! light bakes nothing, a `Mixed` one bakes its bounce, a `Baked` one its bounce and
//! its direct light. Skips with no adapter, like every in-crate GPU test.

use glam::Vec3;

use crate::components::{LightComponent, LightMode, LightType, MaterialAsset};
use crate::scene::authoring::{primitive_mesh_component, Primitive};
use crate::scene::lighting::sh::Sh9;
use crate::scene::{MaterialComponent, Scene};

const RES: u32 = 16;

/// A dark scene: a white static floor slab under a probe at y = 1, and, unless
/// `mode` is `None`, a point light above at y = 3.
fn floor_scene(mode: Option<LightMode>) -> Scene {
    let mut scene = Scene::new();
    scene.ambient_intensity = 0.0;
    scene.materials.insert(
        "white".into(),
        MaterialAsset {
            base_color: [1.0; 3],
            ..MaterialAsset::default()
        },
    );
    let floor = scene.add_entity("Floor".into());
    scene
        .world
        .set_mesh(floor, primitive_mesh_component(Primitive::Box));
    scene.world.set_static(floor, true);
    scene.world.set_material(
        floor,
        Some(MaterialComponent {
            material: "white".into(),
        }),
    );
    {
        let mut t = scene.world.transform_mut(floor).unwrap();
        t.position = Vec3::new(0.0, -0.5, 0.0);
        t.scale = Vec3::new(20.0, 1.0, 20.0);
    }
    if let Some(mode) = mode {
        let lamp = scene.add_entity("Lamp".into());
        scene.world.transform_mut(lamp).unwrap().position = Vec3::new(0.0, 3.0, 0.0);
        let light = LightComponent {
            light_type: LightType::Point,
            color: Vec3::ONE,
            intensity: 4.0,
            range: 20.0,
            inner_cone: 0.0,
            outer_cone: 0.0,
            cast_shadows: false,
            mode,
        };
        scene.world.set_light(lamp, Some(light));
    }
    scene.probes.add_probe(Vec3::new(0.0, 1.0, 0.0));
    scene
}

/// The baked probe, or `None` with no adapter.
fn bake(mode: Option<LightMode>) -> Option<Sh9> {
    let mut renderer = crate::render::test_gpu::headless_or_skip(RES, RES)?;
    let mut scene = floor_scene(mode);
    renderer.bake_probes(&mut scene, RES);
    Some(scene.probes.probes[0].sh)
}

#[test]
fn gpu_probe_bake_light_modes() {
    let Some(unlit) = bake(None) else {
        return;
    };
    let [realtime, mixed, baked] = LightMode::ALL.map(|m| bake(Some(m)).expect("adapter"));
    let (down, up) = (
        |sh: &Sh9| sh.eval(Vec3::NEG_Y).x,
        |sh: &Sh9| sh.eval(Vec3::Y).x,
    );

    assert_eq!(
        realtime, unlit,
        "a Realtime light bakes nothing into the probe"
    );
    assert!(
        down(&mixed) > down(&unlit) + 0.05,
        "a Mixed light's bounce off the floor: {} vs {}",
        down(&mixed),
        down(&unlit)
    );
    assert!(
        (down(&baked) - down(&mixed)).abs() < down(&mixed) * 0.25,
        "a Baked light bounces like a Mixed one: {} vs {}",
        down(&baked),
        down(&mixed)
    );
    // Its direct light comes from above: 4 / (2² + 1) at the probe, facing up.
    assert!(
        up(&baked) - up(&mixed) > 0.6,
        "a Baked light's direct light is in the probe: {} vs {}",
        up(&baked),
        up(&mixed)
    );
}
