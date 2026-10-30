//! The per-cascade static-bake cache (#355, #435), on the GPU: a still camera re-draws
//! no static caster, a nudge inside the snap grid re-draws none either, and a real
//! move re-bakes the cascades it shifted. Skips when no adapter is present. The bake's
//! key moves only with a casting mode that changes the shadow (#1571), or a static
//! Cutout's clip terms (#1594).

use glam::Vec3;

use super::statics_key;
use crate::components::ShadowCasting;
use crate::render::{RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::authoring::{create_entity, Primitive};
use crate::scene::{Camera, Scene};

/// One static box under the default sun, and nothing that moves.
fn static_yard() -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    create_entity(&mut scene, "Sun", Some(Primitive::DirectionalLight));
    let id = create_entity(&mut scene, "Crate", Some(Primitive::Box));
    scene.world.set_static(id, true);
    scene
}

/// Shadow draws submitted rendering `scene` from `position`.
fn shadow_draws(r: &mut Renderer, view: &mut RenderView, scene: &Scene, position: Vec3) -> u32 {
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(position, -90.0, -10.0);
    r.render(view, scene, &cam, &out, false);
    r.frame_counters.shadow_draws
}

#[test]
fn gpu_static_casters_are_baked_once_per_cascade_position() {
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(48, 32) else {
        return;
    };
    let mut view = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, 48, 32);
    let scene = static_yard();
    let home = Vec3::new(0.0, 2.0, 6.0);

    assert!(
        shadow_draws(&mut r, &mut view, &scene, home) > 0,
        "first frame bakes"
    );
    assert_eq!(
        shadow_draws(&mut r, &mut view, &scene, home),
        0,
        "still: cached"
    );
    let nudged = home + Vec3::new(0.01, 0.0, 0.01);
    assert_eq!(
        shadow_draws(&mut r, &mut view, &scene, nudged),
        0,
        "inside the snap grid"
    );
    let moved = home + Vec3::new(0.0, 0.0, 20.0);
    assert!(
        shadow_draws(&mut r, &mut view, &scene, moved) > 0,
        "a moved cascade re-bakes"
    );
}

#[test]
fn only_a_mode_that_changes_the_shadow_moves_the_bake_key() {
    let mut scene = static_yard();
    let crate_id = scene.world.find_by_name("Crate").expect("the crate");
    let mut key_as = |mode| {
        scene.world.mesh_mut(crate_id).unwrap().cast_shadows = mode;
        statics_key(&scene)
    };
    let on = key_as(ShadowCasting::On);
    assert_eq!(
        key_as(ShadowCasting::ShadowsOnly),
        on,
        "casts exactly as On"
    );
    assert_ne!(key_as(ShadowCasting::Off), on);
    assert_ne!(key_as(ShadowCasting::TwoSided), on);
}

/// A static Cutout caster's clip terms re-bake its shadow when edited (#1594); an
/// Opaque one's mapping, which its shadow never reads, keeps the bake.
#[test]
fn a_static_cutouts_mapping_moves_the_bake_key() {
    use crate::components::{RenderMode, UvMapping};
    use crate::scene::authoring::material;
    let mut scene = static_yard();
    let crate_id = scene.world.find_by_name("Crate").expect("the crate");
    let key = material::ensure_material_key(&mut scene, crate_id).unwrap();
    let mut key_with = |edit: &dyn Fn(&mut crate::components::MaterialAsset)| {
        edit(scene.materials.get_mut(&key).unwrap());
        statics_key(&scene)
    };
    let opaque = key_with(&|_| {});
    let tiled = key_with(&|m| m.tiling = [4.0, 4.0]);
    assert_eq!(tiled, opaque, "an Opaque caster's shadow reads no map");
    let cutout = key_with(&|m| m.render_mode = RenderMode::Cutout);
    assert_ne!(cutout, opaque, "Cutout clips its shadow");
    assert_ne!(key_with(&|m| m.offset = [0.5, 0.0]), cutout);
    let offset = key_with(&|_| {});
    assert_ne!(key_with(&|m| m.uv_mapping = UvMapping::Triplanar), offset);
    let triplanar = key_with(&|_| {});
    assert_ne!(key_with(&|m| m.world_size = 3.0), triplanar);
    let sized = key_with(&|_| {});
    assert_ne!(key_with(&|m| m.alpha_cutoff = 0.25), sized);
}

/// The digest numbers a Cutout by its slot index, unresolved slots counted, so the
/// slot it names is the one the shadow clips: moving the Cutout past an unresolved
/// slot moves the key.
#[test]
fn a_cutouts_slot_index_counts_unresolved_slots() {
    use crate::components::RenderMode;
    use crate::scene::authoring::material;
    let mut scene = static_yard();
    let crate_id = scene.world.find_by_name("Crate").expect("the crate");
    let key = material::ensure_material_key(&mut scene, crate_id).unwrap();
    scene.materials.get_mut(&key).unwrap().render_mode = RenderMode::Cutout;
    let mut key_as = |slots: Vec<String>| {
        scene.world.material_mut(crate_id).unwrap().materials = slots;
        statics_key(&scene)
    };
    let first = key_as(vec![key.clone(), "missing".into()]);
    assert_ne!(key_as(vec!["missing".into(), key.clone()]), first);
}
