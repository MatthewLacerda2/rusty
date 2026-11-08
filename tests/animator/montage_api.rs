//! The `Animator` montage surface (#1894): a clip plays as a montage with no
//! graph, the reads see it, an unknown name warns and returns `false`, a bad
//! rate is a Lua error, and stopping an empty slot returns `false`.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::asset::anim_data::AnimationClip;
use rusty::components::MeshComponent;
use rusty::scene::Scene;
use rusty::scripting::{ConsoleLogs, LogLevel};

use super::{run, scene_with_animator};

fn with_flinch() -> (Rc<RefCell<Scene>>, u32) {
    let (scene, id) = scene_with_animator();
    let clip = AnimationClip {
        name: "Flinch".to_string(),
        tracks: Vec::new(),
        duration: 1.0,
    };
    let mesh = MeshComponent {
        clips: vec![clip],
        ..MeshComponent::new("Asset", Vec::new(), Vec::new())
    };
    scene.borrow_mut().world.set_mesh(id, Some(mesh));
    (scene, id)
}

#[test]
fn a_clip_plays_as_a_montage_and_the_reads_see_it() {
    let (scene, id) = with_flinch();
    assert_eq!(
        run(&scene, &format!("Animator.IsMontagePlaying({id})")),
        "false"
    );
    assert_eq!(
        run(&scene, &format!("Animator.PlayMontage({id}, 'Flinch')")),
        "true"
    );
    let read = format!("select(2, Animator.GetMontage({id}, 'DefaultSlot'))");
    assert_eq!(run(&scene, &read), "Default");
    let read = format!("Animator.GetMontage({id}, 'DefaultSlot')");
    assert_eq!(run(&scene, &read), "Flinch");
    assert_eq!(
        run(
            &scene,
            &format!("Animator.IsMontagePlaying({id}, 'DefaultSlot')")
        ),
        "true"
    );
    assert!(scene.borrow().world.animator(id).unwrap().is_playing);
    assert_eq!(
        run(&scene, &format!("Animator.JumpToSection({id}, 'Default')")),
        "true"
    );
    assert_eq!(
        run(
            &scene,
            &format!("Animator.SetNextSection({id}, 'Default', 'Default')")
        ),
        "true"
    );
    assert_eq!(
        run(
            &scene,
            &format!("Animator.SetNextSection({id}, 'Default', 'Nope')")
        ),
        "false"
    );
    assert_eq!(
        run(
            &scene,
            &format!("Animator.StopMontage({id}, 'DefaultSlot', 0)")
        ),
        "true"
    );
    assert_eq!(
        run(&scene, &format!("Animator.IsMontagePlaying({id})")),
        "false"
    );
    assert_eq!(run(&scene, &format!("Animator.StopMontage({id})")), "false");
    run(&scene, &format!("Animator.PlayMontage({id}, 'Flinch')"));
    run(&scene, &format!("Animator.SetGraph({id}, '')"));
    let anim = scene
        .borrow()
        .world
        .animator(id)
        .unwrap()
        .montages
        .is_empty();
    assert!(anim, "a new graph empties every slot, firing nothing");
}

#[test]
fn an_unknown_montage_warns_and_returns_false() {
    let (scene, id) = with_flinch();
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let lua = Lua::new();
    rusty::api::animator::register(&lua, bind(&lua, &scene), bind(&lua, &console)).unwrap();
    let mark = console.borrow().logged();
    let started: bool = lua
        .load(format!("return Animator.PlayMontage({id}, 'Flich')"))
        .eval()
        .unwrap();
    assert!(!started);
    let console = console.borrow();
    let added: Vec<_> = console.since(mark).collect();
    assert_eq!(added.len(), 1, "{added:?}");
    assert_eq!(added[0].1, LogLevel::Warning);
    assert!(added[0].0.contains("'Flich'"), "{added:?}");
}

#[test]
fn a_non_positive_rate_is_a_lua_error_naming_it() {
    let (scene, id) = with_flinch();
    let console = Rc::new(RefCell::new(ConsoleLogs::new()));
    let lua = Lua::new();
    rusty::api::animator::register(&lua, bind(&lua, &scene), bind(&lua, &console)).unwrap();
    let script = format!("Animator.PlayMontage({id}, 'Flinch', {{ rate = 0 }})");
    let err = lua.load(script).exec().unwrap_err().to_string();
    assert!(err.contains("rate"), "{err}");
}
