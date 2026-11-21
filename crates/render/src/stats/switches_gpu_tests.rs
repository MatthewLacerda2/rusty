//! Each measurement switch and graphics option takes away its own counter on a
//! primitive scene with every feature, and leaves the others' as they were. A fresh
//! renderer a case: a page or cube one case loaded stays resident.

use super::{RenderSwitch, RenderSwitches};
use crate::test_gpu::ssil;
use crate::RenderCounters;
use glam::{Quat, Vec3};
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
    let probe = (scene.reflection_probes).add(Vec3::new(0.0, 1.0, -4.0), Vec3::splat(20.0));
    scene.reflection_probes.set_cubemap(probe, cube);
    scene
}

/// What each feature costs, as the counter its switch or option takes away.
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// [`costs`] with every switch on but `switch`, in the Scene view when `editor`.
fn switched(switch: RenderSwitch, editor: bool) -> Costs {
    let mut s = RenderSwitches::ON;
    s.set(switch, false);
    costs(s, GraphicsOptions::default(), editor).unwrap()
}

/// The costs of [`everything`]'s second frame on a fresh renderer drawing with
/// `switches` and `options`, in the Scene view (which ignores them) when `editor`.
fn costs(switches: RenderSwitches, options: GraphicsOptions, editor: bool) -> Option<Costs> {
    let mut r = ssil::renderer()?;
    (r.switches, r.graphics_options) = (switches, options);
    let (scene, mut view) = (everything(), ssil::view(&r));
    let out = view.color_target_view().unwrap();
    for _ in 0..2 {
        r.render(&mut view, &scene, &ssil::eye(0.0), &out, editor);
    }
    Some(r.frame_counters.into())
}

/// The costs with everything on, each checked present; `None` with no adapter.
fn baseline() -> Option<Costs> {
    let all = costs(RenderSwitches::ON, GraphicsOptions::default(), false)?;
    let counted = [all.ssao_samples, all.ssil_samples, all.lightmap_bytes];
    let counted = counted.into_iter().chain([all.light_cluster_refs]);
    assert!(all.shadow_draws > 0 && counted.min() > Some(0), "{all:?}");
    Some(all)
}

impl Costs {
    /// These costs with one taken away by `zero`.
    fn without(mut self, zero: fn(&mut Costs)) -> Costs {
        zero(&mut self);
        self
    }
}

#[test]
fn gpu_each_switch_takes_away_its_own_counter_and_no_other() {
    let Some(all) = baseline() else {
        return;
    };
    let shadowless = switched(RenderSwitch::Shadows, false);
    assert_eq!(shadowless, all.without(|c| c.shadow_draws = 0));
    let unbaked = switched(RenderSwitch::Lightmaps, false);
    assert_eq!(unbaked, all.without(|c| c.lightmap_bytes = 0));
    let unlit = switched(RenderSwitch::ClusteredLights, false);
    assert_eq!(unlit, all.without(|c| c.light_cluster_refs = 0));
    // The array shrinks to its stand-in cube, which is not nothing.
    let probeless = switched(RenderSwitch::ReflectionProbes, false);
    let bytes = |c: Costs| c.reflection_bytes;
    assert!(bytes(probeless) < bytes(all), "{probeless:?} {all:?}");
    let rest = |c: Costs| c.without(|c| c.reflection_bytes = 0);
    assert_eq!(rest(probeless), rest(all));
}

#[test]
fn gpu_the_ssao_and_ssil_options_and_switches_take_away_their_own_counter_and_no_other() {
    let Some(all) = baseline() else {
        return;
    };
    let option_off = |option| {
        let mut o = GraphicsOptions::default();
        o.set(option, false);
        costs(RenderSwitches::ON, o, false).unwrap()
    };
    let no_ao = option_off(GraphicsOption::Ssao);
    assert_eq!(no_ao, all.without(|c| c.ssao_samples = 0));
    let no_il = option_off(GraphicsOption::Ssil);
    assert_eq!(no_il, all.without(|c| c.ssil_samples = 0));
    // The `ssao` and `ssil` switches take the same, in the Scene view too.
    assert_eq!(switched(RenderSwitch::Ssao, false), no_ao);
    assert_eq!(switched(RenderSwitch::Ssil, false), no_il);
    let editor = costs(RenderSwitches::ON, GraphicsOptions::default(), true).unwrap();
    assert!(
        editor.ssao_samples > 0 && editor.ssil_samples > 0,
        "{editor:?}"
    );
    let no_ao = switched(RenderSwitch::Ssao, true);
    assert_eq!(no_ao, editor.without(|c| c.ssao_samples = 0));
    let no_il = switched(RenderSwitch::Ssil, true);
    assert_eq!(no_il, editor.without(|c| c.ssil_samples = 0));
}
