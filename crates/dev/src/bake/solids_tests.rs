//! What placement reads as solid (#1737): the static colliders and the opaque
//! static meshes, so a ceiling built as a render mesh alone still roofs its room.

use glam::Vec3;

use super::{static_solids, survey};
use rusty_scene::authoring::{create_entity, Primitive};
use rusty_scene::lighting::placement::Occupancy;
use rusty_scene::lighting::placement::{plan_reflection_probes, ReflectionPlacement, Solid};
use rusty_scene::Scene;

/// A 12 × 12 m floor and four walls with colliders, under a ceiling 4 m up that
/// is a box mesh with no collider.
fn room_under_a_mesh_ceiling() -> Scene {
    let mut scene = Scene::new();
    let block = rusty_scene::lighting::placement::test_level::block;
    let (r, h) = (6.0, 4.0);
    block(&mut scene, Vec3::new(-r, -0.2, -r), Vec3::new(r, 0.0, r));
    block(
        &mut scene,
        Vec3::new(-r, 0.0, -r),
        Vec3::new(r, h, -r + 0.3),
    );
    block(&mut scene, Vec3::new(-r, 0.0, r - 0.3), Vec3::new(r, h, r));
    block(
        &mut scene,
        Vec3::new(-r, 0.0, -r),
        Vec3::new(-r + 0.3, h, r),
    );
    block(&mut scene, Vec3::new(r - 0.3, 0.0, -r), Vec3::new(r, h, r));
    let ceiling = create_entity(&mut scene, "Ceiling", Some(Primitive::Box));
    scene.world.set_static(ceiling, true);
    scene.world.set_collider(ceiling, None);
    if let Some(mut t) = scene.world.transform_mut(ceiling) {
        t.position = Vec3::new(0.0, h + 0.1, 0.0);
        t.scale = Vec3::new(2.0 * r, 0.2, 2.0 * r);
    }
    scene
}

#[test]
fn a_mesh_ceiling_roofs_its_room() {
    let scene = room_under_a_mesh_ceiling();
    let solids = static_solids(&scene);
    let occ = Occupancy::build(&solids, 0.5).unwrap();
    assert!(
        occ.ray(Vec3::new(0.0, 1.6, 0.0), Vec3::Y, 10.0).is_some(),
        "the ceiling is solid"
    );
    let site = survey(&scene, None, 0.5).expect("a site");
    let plans = plan_reflection_probes(&scene, &site, &ReflectionPlacement::default(), &[]);
    assert!(!plans.is_empty());
    assert!(
        plans.iter().all(|p| p.box_projection),
        "roofed, not open air: {plans:?}"
    );
    let light = site.plan_light_probes(4.0, &[]);
    assert!(
        !light.is_empty() && light.iter().all(|p| p.y < 4.0),
        "a light probe over the ceiling"
    );
}

/// A transparent mesh (a pool's water, a pane of glass) is not solid.
#[test]
fn a_transparent_mesh_is_not_solid() {
    use rusty_components::{MaterialAsset, MaterialComponent, RenderMode};
    let mut scene = room_under_a_mesh_ceiling();
    let ceiling = scene.world.ids_with_mesh()[0];
    let glass = MaterialAsset {
        render_mode: RenderMode::Transparent,
        ..MaterialAsset::default()
    };
    scene.materials.insert("glass".into(), glass);
    scene
        .world
        .set_material(ceiling, Some(MaterialComponent::new("glass")));
    let solids: Vec<Solid> = static_solids(&scene);
    let triangles: usize = solids
        .iter()
        .filter(|s| !s.convex)
        .map(|s| s.triangles.len())
        .sum();
    assert_eq!(triangles, 0, "the glass's triangles are left out");
}
