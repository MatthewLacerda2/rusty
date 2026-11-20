//! Each measurement switch, and each graphics option the bench reaches beside
//! them, takes away its own counter on a primitive scene that has every feature
//! (a sun, a shadowing lamp, a dynamic caster, SSAO, SSIL, a lightmap page, a reflection probe) and
//! leaves the others' as they were. A fresh renderer a case: a page or cube one
//! case loaded stays resident, and would count in the next.

use glam::{Quat, Vec3};

use super::{RenderSwitch, RenderSwitches};
use crate::test_gpu::ssil;
use crate::RenderCounters;
use rusty_core::settings::graphics_options::{GraphicsOption, GraphicsOptions};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::lighting::lightmap_set::LightmapEntry;
use rusty_scene::Scene;

/// The SSIL test room with SSAO on, the lamp shadowing, a sun, the floor
/// lightmapped and a reflection probe around it all.
fn everything() -> Scene {
    let (mut scene, lamp) = ssil::scene(ssil::on(1.0), ssil::LAMP);
    let cam = scene.world.ids_with_camera()[0];
    let mut volume = scene.world.post_processing(cam).unwrap().clone();
    volume.ssao.active = true;
    scene.world.set_post_processing(cam, Some(volume));
    scene.world.light_mut(lamp).unwrap().cast_shadows = true;
    let sun = create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    scene.world.transform_mut(sun).unwrap().rotation = Quat::from_rotation_x(-1.0);
    // A dynamic caster: the statics are drawn into the maps once, then cached.
    let crate_id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    scene.world.transform_mut(crate_id).unwrap().position = Vec3::new(0.5, 0.5, -4.0);
    let floor = scene.find_entity_by_name("floor").unwrap();
    let page = rusty_test_support::temp::dir().join("switches_page.png");
    image::RgbaImage::new(4, 4).save(&page).unwrap();
    scene.lightmaps.pages = vec![page.to_string_lossy().into_owned()];
    scene.lightmaps.insert(LightmapEntry {
        entity: floor,
        page: 0,
        scale_offset: [1.0, 1.0, 0.0, 0.0],
    });
    let cube = crate::ibl::reflection_probes::gpu_tests::cube_file("switches.ktx2", 4, 90);
    let probes = &mut scene.reflection_probes;
    let probe = probes.add(Vec3::new(0.0, 1.0, -4.0), Vec3::splat(20.0));
    probes.set_cubemap(probe, cube);
    scene
}

/// What each feature costs, as the counter its switch or option takes away.
#[derive(Debug, PartialEq)]
struct Costs {
    shadow_draws: u32,
    ssao_samples: u64,
    ssil_samples: u64,
    lightmap_bytes: u64,
    reflection_bytes: u64,
    light_cluster_refs: u64,
}

impl From<RenderCounters> for Costs {
    fn from(c: RenderCounters) -> Self {
        Self {
            shadow_draws: c.shadow_draws,
            ssao_samples: c.ssao_samples,
            ssil_samples: c.ssil_samples,
            lightmap_bytes: c.lightmap_bytes,
            reflection_bytes: c.reflection_bytes,
            light_cluster_refs: c.light_cluster_refs,
        }
    }
}

/// The costs of [`everything`]'s second frame on a fresh renderer drawing with
/// `switches` and `options`; `None` with no adapter.
fn costs(switches: RenderSwitches, options: GraphicsOptions) -> Option<Costs> {
    let mut r = ssil::renderer()?;
    (r.switches, r.graphics_options) = (switches, options);
    let (scene, mut view) = (everything(), ssil::view(&r));
    let out = view.color_target_view().unwrap();
    for _ in 0..2 {
        r.render(&mut view, &scene, &ssil::eye(0.0), &out, false);
    }
    Some(r.frame_counters.into())
}

#[test]
fn gpu_each_switch_takes_away_its_own_counter_and_no_other() {
    let Some(all) = costs(RenderSwitches::ON, GraphicsOptions::default()) else {
        return;
    };
    let zeroed = |field: fn(&mut Costs)| {
        let mut want = Costs { ..all };
        field(&mut want);
        want
    };
    assert!(
        all.shadow_draws > 0 && all.ssao_samples > 0 && all.ssil_samples > 0,
        "{all:?}"
    );
    assert!(
        all.lightmap_bytes > 0 && all.light_cluster_refs > 0,
        "{all:?}"
    );
    let switched = |switch| {
        let mut s = RenderSwitches::ON;
        s.set(switch, false);
        costs(s, GraphicsOptions::default()).unwrap()
    };
    assert_eq!(
        switched(RenderSwitch::Shadows),
        zeroed(|c| c.shadow_draws = 0)
    );
    assert_eq!(
        switched(RenderSwitch::Lightmaps),
        zeroed(|c| c.lightmap_bytes = 0)
    );
    assert_eq!(
        switched(RenderSwitch::ClusteredLights),
        zeroed(|c| c.light_cluster_refs = 0)
    );
    let probeless = switched(RenderSwitch::ReflectionProbes);
    assert!(
        probeless.reflection_bytes < all.reflection_bytes,
        "{probeless:?} {all:?}"
    );
    let reflection_bytes = probeless.reflection_bytes;
    assert_eq!(
        probeless,
        Costs {
            reflection_bytes,
            ..all
        }
    );
    let option_off = |option| {
        let mut o = GraphicsOptions::default();
        o.set(option, false);
        costs(RenderSwitches::ON, o).unwrap()
    };
    assert_eq!(
        option_off(GraphicsOption::Ssao),
        zeroed(|c| c.ssao_samples = 0)
    );
    assert_eq!(
        option_off(GraphicsOption::Ssil),
        zeroed(|c| c.ssil_samples = 0)
    );
}
