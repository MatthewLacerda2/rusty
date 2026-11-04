//! `Animator.Play` / `Crossfade` and the console (#1718): a successful call logs
//! nothing, and a clip the entity's mesh lacks warns.

use std::cell::RefCell;

use mlua::Lua;
use rusty::asset::anim_data::AnimationClip;
use rusty::components::MeshComponent;
use rusty::scene::Scene;
use rusty::scripting::{ConsoleLogs, LogLevel};

use super::scene_with_animator;

/// Run `script` against `scene` with `console` as the namespace's console.
fn run_with(scene: &RefCell<Scene>, console: &RefCell<ConsoleLogs>, script: &str) {
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::animator::register(&lua, s, scene, console).unwrap();
        lua.load(script).exec()
    })
    .unwrap();
}

fn give_walk_clip(scene: &RefCell<Scene>, id: u32) {
    scene.borrow_mut().world.set_mesh(
        id,
        Some(MeshComponent {
            clips: vec![AnimationClip {
                name: "Walk".to_string(),
                tracks: Vec::new(),
                duration: 1.0,
            }],
            ..MeshComponent::new("Asset", Vec::new(), Vec::new())
        }),
    );
}

#[test]
fn a_successful_play_or_crossfade_logs_nothing() {
    let (scene, id) = scene_with_animator();
    let console = RefCell::new(ConsoleLogs::new());
    // No mesh yet: a Play issued before it arrives stays quiet (#1386).
    run_with(&scene, &console, &format!("Animator.Play({id}, 'Walk')"));
    give_walk_clip(&scene, id);
    let mark = console.borrow().logged();
    let script = format!(
        "for _ = 1, 20 do Animator.Play({id}, 'Walk') end Animator.Crossfade({id}, 'Walk', 0.2)"
    );
    run_with(&scene, &console, &script);
    assert_eq!(console.borrow().logged(), mark, "a successful call logged");
}

#[test]
fn a_clip_the_mesh_lacks_warns_once_per_run_of_repeats() {
    let (scene, id) = scene_with_animator();
    give_walk_clip(&scene, id);
    let console = RefCell::new(ConsoleLogs::new());
    let mark = console.borrow().logged();
    run_with(&scene, &console, &format!("Animator.Play({id}, 'Wlak')"));
    run_with(&scene, &console, &format!("Animator.Play({id}, 'Wlak')"));
    let console = console.borrow();
    let added: Vec<_> = console.since(mark).collect();
    assert_eq!(added.len(), 1, "repeats collapse into one entry: {added:?}");
    assert_eq!(added[0].1, LogLevel::Warning);
    assert!(
        added[0].0.contains("'Wlak'") && added[0].0.contains("(×2)"),
        "{added:?}"
    );
    drop(console);
    // The clip still takes over, as documented.
    assert_eq!(
        scene.borrow().world.animator(id).unwrap().base.current_clip,
        "Wlak"
    );
}

#[test]
fn crossfade_into_a_clip_the_mesh_lacks_warns() {
    let (scene, id) = scene_with_animator();
    give_walk_clip(&scene, id);
    let console = RefCell::new(ConsoleLogs::new());
    let mark = console.borrow().logged();
    run_with(
        &scene,
        &console,
        &format!("Animator.Crossfade({id}, 'Ghost', 0.2)"),
    );
    let console = console.borrow();
    let added: Vec<_> = console.since(mark).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert!(added[0].0.starts_with("Animator.Crossfade:"), "{added:?}");
}
