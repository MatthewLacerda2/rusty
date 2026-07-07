//! src/dev/demo_scene.rs — headless demo scene builder.
//!
//! Mirrors the windowed demo scene from `main.rs` (entity ids match: Player = 2,
//! Enemy_1 = 5) but without any GPU/mesh upload concerns, so the harness can play
//! the same world the editor shows. Deterministic: no RNG, fixed positions.

use glam::Vec3;

use crate::render::gpu::mesh as primitives;
use crate::scene::{
    AnimatorComponent, ColliderComponent, ColliderShape, CollisionDetection, DirtyFlag,
    MaterialAsset, MaterialComponent, MeshComponent, RigidBodyComponent, Scene, ScriptComponent,
};

/// Bundled default player brain (movement + camera + weapon), seeded next to
/// `bot.lua` and attached to the Player. Game logic, not engine logic.
pub const PLAYER_CONTROLLER_SCRIPT: &str = "project/assets/scripts/player_controller.lua";

fn box_collider(size: Vec3) -> ColliderComponent {
    ColliderComponent {
        active: true,
        shape: ColliderShape::Box { size },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    }
}

fn mesh(primitive_type: &str, data: (Vec<primitives::Vertex>, Vec<u32>)) -> MeshComponent {
    MeshComponent {
        primitive_type: primitive_type.to_string(),
        asset_ref: None,
        vertices: data.0,
        indices: data.1,
        bind_palette: Vec::new(),
        skin: None,
        clips: Vec::new(),
        pose_palette: Vec::new(),
        is_dirty: DirtyFlag::new(true),
    }
}

/// Create a colour-only library material (no albedo map, just a tint) under `key`
/// and return the reference an entity carries to it. The data lives in the scene's
/// material library; the entity holds only the name.
fn tint(scene: &mut Scene, key: &str, color: [f32; 3]) -> MaterialComponent {
    scene.materials.insert(
        key.to_string(),
        MaterialAsset {
            base_color: color,
            ..MaterialAsset::default()
        },
    );
    MaterialComponent {
        material: key.to_string(),
    }
}

fn kinematic_body() -> RigidBodyComponent {
    RigidBodyComponent {
        active: true,
        is_kinematic: true,
        mass: 80.0,
        velocity: Vec3::ZERO,
        angular_velocity: Vec3::ZERO,
        use_gravity: false,
        collision_detection: CollisionDetection::Discrete,
    }
}

/// Build the standard demo scene used by both the editor and the harness.
/// `bot_script` is the path to the enemy's Lua brain (may be empty to skip).
///
/// Each entity's `RefMut` guard is scoped so it drops before the next `Scene`
/// call — required now that the Scene is hecs-backed and hands out borrow guards.
pub fn build(scene: &mut Scene, bot_script: &str) {
    add_floor(scene);
    add_player(scene);
    add_walls(scene);
    add_enemy(scene, bot_script);
    scene.update_all_colliders();
}

/// 1 — Floor (id 1)
fn add_floor(scene: &mut Scene) {
    let floor_id = scene.add_entity("Floor_Plane".to_string());
    scene.world.transform_mut(floor_id).unwrap().scale = Vec3::new(2.5, 1.0, 2.5);
    scene.world.set_static(floor_id, true);
    scene.world.set_mesh(
        floor_id,
        Some(mesh("Plane", primitives::generate_plane(15.0, 15.0))),
    );
    scene
        .world
        .set_collider(floor_id, Some(box_collider(Vec3::new(15.0, 0.1, 15.0))));
}

/// 2 — Player (id 2)
fn add_player(scene: &mut Scene) {
    let player_id = scene.add_entity("Player".to_string());
    // Blue tint via a library material (empty albedo => colour-only). The renderer
    // reads colour from the referenced material, never from the entity name.
    let material = tint(scene, "player_material", [0.3, 0.6, 1.0]);
    scene.world.transform_mut(player_id).unwrap().position = Vec3::new(0.0, 1.5, -6.0);
    scene.world.set_mesh(
        player_id,
        Some(mesh(
            "Cylinder",
            primitives::generate_cylinder(
                Vec3::new(0.0, -0.8, 0.0),
                Vec3::new(0.0, 0.8, 0.0),
                0.5,
                12,
            ),
        )),
    );
    scene.world.set_collider(
        player_id,
        Some(ColliderComponent {
            shape: ColliderShape::Cylinder {
                radius: 0.5,
                height: 1.6,
            },
            ..box_collider(Vec3::ONE)
        }),
    );
    scene.world.set_rigidbody(player_id, Some(kinematic_body()));
    scene.world.set_material(player_id, Some(material));
    // The Player's movement + camera + weapon are NOT engine code: they live in
    // the bundled default player_controller.lua, attached here like any script.
    scene
        .world
        .scripts_mut(player_id)
        .unwrap()
        .push(ScriptComponent {
            path: PLAYER_CONTROLLER_SCRIPT.to_string(),
            is_loaded: false,
            ..Default::default()
        });
}

/// 3, 4 — Obstacle walls
fn add_walls(scene: &mut Scene) {
    let wall1_id = scene.add_entity("Obstacle_Wall_Left".to_string());
    {
        let mut t = scene.world.transform_mut(wall1_id).unwrap();
        t.position = Vec3::new(3.0, 1.0, 2.0);
        t.scale = Vec3::new(1.0, 2.0, 4.0);
    }
    scene.world.set_static(wall1_id, true);
    scene.world.set_mesh(
        wall1_id,
        Some(mesh("Box", primitives::generate_box(1.0, 1.0, 1.0))),
    );
    scene
        .world
        .set_collider(wall1_id, Some(box_collider(Vec3::ONE)));

    let wall2_id = scene.add_entity("Obstacle_Wall_Right".to_string());
    {
        let mut t = scene.world.transform_mut(wall2_id).unwrap();
        t.position = Vec3::new(-3.0, 1.0, 4.0);
        t.scale = Vec3::new(4.0, 2.0, 1.0);
    }
    scene.world.set_static(wall2_id, true);
    scene.world.set_mesh(
        wall2_id,
        Some(mesh("Box", primitives::generate_box(1.0, 1.0, 1.0))),
    );
    scene
        .world
        .set_collider(wall2_id, Some(box_collider(Vec3::ONE)));
}

/// 5 — Enemy_1 (id 5)
fn add_enemy(scene: &mut Scene, bot_script: &str) {
    let enemy_id = scene.add_entity("Enemy_1".to_string());
    scene.world.transform_mut(enemy_id).unwrap().position = Vec3::new(8.0, 1.0, 8.0);
    scene.world.set_mesh(
        enemy_id,
        Some(mesh("Box", primitives::generate_box(1.3, 2.0, 1.3))),
    );
    scene
        .world
        .set_collider(enemy_id, Some(box_collider(Vec3::new(1.3, 2.0, 1.3))));
    scene.world.set_rigidbody(enemy_id, Some(kinematic_body()));
    scene.world.set_animator(
        enemy_id,
        Some(AnimatorComponent {
            current_clip: "Walk".to_string(),
            speed: 3.0,
            is_playing: true,
            ..Default::default()
        }),
    );
    if !bot_script.is_empty() {
        scene
            .world
            .scripts_mut(enemy_id)
            .unwrap()
            .push(ScriptComponent {
                path: bot_script.to_string(),
                is_loaded: false,
                ..Default::default()
            });
    }
}
