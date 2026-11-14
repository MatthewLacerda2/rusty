//! Root motion of a Blender export through the real play loop (#1822): the
//! animated mesh sits under an `Armature` entity turned +90° about X and scaled
//! 0.01, so its own space is Z-up centimetres. A clip that travels 2 m along
//! model +Z moves it 2 m along world +Z with the hips held under it, and a turn
//! keeps the model upright.

use std::cell::RefCell;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;

use glam::Vec3;

use crate::game::game_tests::{world_with, DT};
use rusty_asset::humanoid::rig_fixture::{blender_armature, lunge, rig, Shape};
use rusty_components::{AnimatorComponent, MeshComponent, Playback, TransformComponent};
use rusty_scene::Scene;

/// The exported rig's body playing the lunge with root motion applied, under
/// its `Armature` entity. Returns the scene and the body.
fn exported(travel: Vec3, turn: f32) -> (Scene, u32) {
    let mut scene = Scene::new();
    let armature = scene.add_entity("Armature".to_string());
    *scene.world.transform_mut(armature).unwrap() =
        TransformComponent::from_matrix(blender_armature());
    let body = scene.add_entity("Body".to_string());
    scene.set_parent(body, Some(armature)).unwrap();
    let skin = rig(Shape {
        armature: blender_armature(),
        ..Shape::default()
    });
    let mesh = MeshComponent {
        clips: vec![lunge(&skin, travel, turn)],
        skin: Some(skin),
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    scene.world.set_mesh(body, Some(mesh));
    scene.sync_skeletons();
    let anim = AnimatorComponent {
        base: Playback {
            current_clip: "Lunge".to_string(),
            ..Playback::default()
        },
        is_playing: true,
        apply_root_motion: true,
        ..AnimatorComponent::default()
    };
    scene.world.set_animator(body, Some(anim));
    (scene, body)
}

/// Play 1.5 s; each tick, the body's and the hips' world positions.
fn play(scene: &Rc<RefCell<Scene>>, body: u32) -> Vec<(Vec3, Vec3)> {
    let mut gw = world_with(Rc::clone(scene));
    gw.set_playing(true);
    (0..90)
        .map(|_| {
            gw.tick(DT);
            let s = scene.borrow();
            let hips = s.find_bone(body, "mixamorig:Hips").expect("the hips bone");
            (s.world.world_position(body), s.world.world_position(hips))
        })
        .collect()
}

#[test]
fn two_metres_of_model_travel_move_the_body_two_metres_with_the_hips_under_it() {
    let (scene, body) = exported(Vec3::Z * 2.0, 0.0);
    let scene = Rc::new(RefCell::new(scene));
    let trace = play(&scene, body);
    let (end, _) = trace[trace.len() - 1];
    assert!(
        end.distance(Vec3::Z * 2.0) < 1e-3,
        "moved 2 m along +Z: {end}"
    );
    let (first_body, first_hips) = trace[0];
    let under = (first_hips - first_body) * Vec3::new(1.0, 0.0, 1.0);
    for (b, h) in &trace {
        let off = (*h - *b) * Vec3::new(1.0, 0.0, 1.0);
        assert!(
            off.distance(under) < 1e-3,
            "the hips slid: {off} vs {under}"
        );
    }
    let lift = trace
        .iter()
        .map(|(b, h)| h.y - b.y)
        .fold(f32::MIN, f32::max);
    assert!(
        lift > first_hips.y - first_body.y + 0.05,
        "the lift stays in the pose"
    );
}

#[test]
fn a_quarter_turn_turns_the_body_about_world_up_and_keeps_it_upright() {
    let (scene, body) = exported(Vec3::ZERO, FRAC_PI_2);
    let scene = Rc::new(RefCell::new(scene));
    play(&scene, body);
    let s = scene.borrow();
    let skin = s
        .world
        .mesh(body)
        .and_then(|m| m.skin.clone())
        .expect("skinned");
    let shown = |model: Vec3| {
        let local = skin.mesh_inverse.transform_vector3(model).normalize();
        s.world.transform_direction(body, local)
    };
    let (up, front) = (shown(Vec3::Y), shown(Vec3::Z));
    assert!(up.distance(Vec3::Y) < 1e-3, "upright: {up}");
    assert!(
        front.distance(Vec3::X) < 1e-3,
        "faces +X after a quarter turn: {front}"
    );
}
