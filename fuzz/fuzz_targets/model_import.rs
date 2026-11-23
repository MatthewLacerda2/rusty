#![no_main]
//! Fuzz the model import: a malformed model file must be an import error, never a panic
//! in the sim. Bytes -> `import_slice` -> `ImportedAsset::validate`, the oracle of what
//! the runtime assumes; then the readers that index it: clips sampled across their
//! length, the model spawned (nodes, bones), its palettes, hitboxes and mesh colliders.
//! The corpus seeds are primitive fixtures, so mutation starts past the parser.

use glam::Vec3;
use libfuzzer_sys::fuzz_target;
use rusty::app::animation::{sample_pose, step};
use rusty::asset::gltf::import::import_slice;
use rusty::asset::Imported;
use rusty::components::{AnimatorComponent, ColliderComponent, MeshComponent, Motion};
use rusty::edit::model_instance::instantiate_model;
use rusty::physics::PhysicsWorld;
use rusty::scene::skeleton::HitboxOptions;
use rusty::scene::Scene;

/// Fractions of a clip's length, past the end included.
const AT: [f32; 5] = [0.0, 0.3, 0.5, 1.0, 1.7];

fuzz_target!(|data: &[u8]| {
    let Ok(asset) = import_slice(data) else {
        return;
    };
    if let Err(broken) = asset.validate() {
        panic!("the import broke an invariant: {broken}");
    }
    for sub in &asset.sub_meshes {
        let mesh = MeshComponent {
            skin: sub.skin.clone(),
            clips: sub.clips.clone(),
            ..MeshComponent::new("Asset", sub.vertices.clone(), sub.indices.clone())
        };
        for clip in &sub.clips {
            let mut animator = AnimatorComponent {
                is_playing: true,
                ..AnimatorComponent::default()
            };
            animator.base.play(Motion::Clip(&clip.name));
            for at in AT {
                animator.base.time = clip.duration * at;
                let _ = sample_pose(&animator, &mesh, None);
            }
            step(&mut animator, Some(&mesh), None, 1.0 / 60.0);
        }
    }

    let mut scene = Scene::new();
    instantiate_model(&mut scene, "fuzz.glb", &Imported::new(asset), Vec3::ZERO);
    for id in scene.entity_ids() {
        let Some((lo, hi)) = scene.world.mesh(id).and_then(|m| bounds(&m)) else {
            continue;
        };
        let convex = id % 2 == 0;
        let collider = ColliderComponent::from_mesh_bounds(convex, lo, hi);
        scene.world.set_collider(id, Some(collider));
        if scene.world.mesh(id).is_some_and(|m| m.skin.is_some()) {
            let _ = scene.generate_hitboxes(id, &HitboxOptions::default());
        }
    }
    scene.build_skin_palettes();
    let _ = PhysicsWorld::from_scene(&scene);
});

fn bounds(mesh: &MeshComponent) -> Option<(Vec3, Vec3)> {
    let mut points = mesh.vertices.iter().map(|v| Vec3::from_array(v.position));
    let first = points.next()?;
    Some(points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
}
