//! The rig the animation runtime tests drive: a one-joint `hand` skeleton, its
//! slide clip, an armed character scene, and a play loop through the real tick.

use std::cell::RefCell;
use std::rc::Rc;

use glam::{Mat4, Vec3};
use rusty::app::GameWorld;
use rusty::asset::anim_data::{AnimationClip, Interpolation, JointTrack, Track};
use rusty::asset::mesh_data::{JointTransform, SkinData};
use rusty::components::{AnimatorComponent, MeshComponent};
use rusty::core::input::InputState;
use rusty::navigation::NavigationGraph;
use rusty::scene::{DirtyFlag, Scene, ScriptComponent};
use rusty::scripting::ConsoleLogs;

pub const DT: f32 = 1.0 / 60.0;

/// A one-joint skin, `hand`, whose clip slides it +X over [0, 1] s — the smallest
/// rig that produces a non-identity pose.
fn slide_skin() -> SkinData {
    SkinData {
        inverse_bind: vec![Mat4::IDENTITY],
        bind_global: vec![Mat4::IDENTITY],
        local_bind: vec![JointTransform::default()],
        parents: vec![None],
        joint_nodes: vec![0],
        mesh_inverse: Mat4::IDENTITY,
        names: vec!["hand".to_string()],
    }
}

fn slide_clip() -> AnimationClip {
    let mut tracks = vec![JointTrack::default()];
    tracks[0].translation = Track {
        times: vec![0.0, 1.0],
        values: vec![Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0)],
        interpolation: Interpolation::Linear,
    };
    let mut clip = AnimationClip {
        name: "Slide".to_string(),
        tracks,
        duration: 0.0,
    };
    clip.recompute_duration();
    clip
}

/// A scene with one animated character (its skeleton spawned) and a `Gun` held in
/// its hand; returns the scene with the (character, hand bone, gun) ids.
pub fn armed_scene(script: Option<&str>) -> (Scene, u32, u32, u32) {
    let mut scene = Scene::new();
    let hero = scene.add_entity("Hero".to_string());
    scene.world.set_mesh(
        hero,
        Some(MeshComponent {
            primitive_type: "Asset".to_string(),
            asset_ref: Some("model.glb::Hero".to_string()),
            vertices: Vec::new(),
            indices: Vec::new(),
            bind_palette: vec![Mat4::IDENTITY],
            skin: Some(slide_skin()),
            clips: vec![slide_clip()],
            pose_palette: Vec::new(),
            skeleton: Default::default(),
            is_dirty: DirtyFlag::new(true),
        }),
    );
    let mut anim = AnimatorComponent::default();
    anim.play("Slide".to_string());
    scene.world.set_animator(hero, Some(anim));
    if let Some(source) = script {
        let path = crate::temp::dir().join("rusty_453_late.lua");
        std::fs::write(&path, source).unwrap();
        *scene.world.scripts_mut(hero).unwrap() = vec![ScriptComponent {
            path: path.to_string_lossy().replace('\\', "/"),
            ..Default::default()
        }];
    }
    scene.sync_skeleton(hero);
    let hand = scene
        .find_bone(hero, "hand")
        .expect("the skeleton was spawned");
    let gun = scene.add_entity("Gun".to_string());
    scene.world.transform_mut(gun).unwrap().position = Vec3::new(0.0, 0.5, 0.0);
    scene.set_parent(gun, Some(hand)).unwrap();
    (scene, hero, hand, gun)
}

/// Play `scene` for `ticks` fixed steps through the real schedule.
pub fn play(scene: Scene, ticks: usize) -> Rc<RefCell<Scene>> {
    let scene = Rc::new(RefCell::new(scene));
    let mut gw = GameWorld::new(
        scene.clone(),
        Rc::new(RefCell::new(InputState::new())),
        Rc::new(RefCell::new(NavigationGraph::new(
            -5.0, 5.0, -5.0, 5.0, 1.0,
        ))),
        Rc::new(RefCell::new(ConsoleLogs::new())),
    );
    gw.set_playing(true);
    for _ in 0..ticks {
        gw.tick(DT);
    }
    scene
}
