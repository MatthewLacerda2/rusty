//! A light made while the game plays is live: no bake can take in a light
//! the edit-mode scene never held, so `Baked`, the edit-mode default, would light
//! nothing in a shipped game. It casts a shadow that fades a few ranges from the
//! camera. One made in edit mode stays `Baked` and unshadowed.

use std::cell::RefCell;
use std::rc::Rc;

use rusty::components::LightMode;
use rusty::edit::authoring::{add_component, create_entity, ComponentKind, Primitive};
use rusty::scene::create::RUNTIME_SHADOW_FADE_RANGES;
use rusty::scene::Scene;

use crate::play::{attach_script, playing, run};

/// The muzzle flash: a script's `Update` creates a lamp and adds a light to an
/// empty, and both draw live whether or not the build previews unbaked lights
/// (a harness or a `player`).
#[test]
fn a_light_created_in_update_is_realtime_and_drawn_live() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let gun = scene.borrow_mut().add_entity("Gun".to_string());
    let source = r#"
        local done = false
        return { Update = function()
            if done then return end
            done = true
            Scene.CreateEntity("Flash", "PointLight")
            Scene.AddComponent(Scene.CreateEntity("Spot"), "Light")
        end }
    "#;
    attach_script(&mut scene.borrow_mut(), gun, "flash.lua", source);
    let mut game = playing(scene.clone(), 1.0);
    run(&mut game, 2);
    let scene = scene.borrow();
    for name in ["Flash", "Spot"] {
        let id = scene.world.find_by_name(name).expect("the script made it");
        let light = scene.world.light(id).expect("a light").clone();
        assert_eq!(light.mode, LightMode::Realtime, "{name}");
        assert!(light.cast_shadows, "{name}: a runtime light casts shadows");
        let fade = light.range * RUNTIME_SHADOW_FADE_RANGES;
        assert_eq!(light.shadow_fade_distance, fade, "{name}: and fades them");
        for preview in [true, false] {
            let drawn = scene.drawn_mode(id, &light, preview);
            assert!(drawn.renders_live(false), "{name} preview={preview}");
        }
        assert!(!scene.awaits_bake(id, &light), "{name}: never waits");
    }
}

/// In edit mode both entry points give the defaults: `Baked`, no shadow.
#[test]
fn a_light_created_in_edit_mode_is_baked() {
    let mut scene = Scene::new();
    let lamp = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));
    let empty = scene.add_entity("Empty".to_string());
    assert!(add_component(&mut scene, empty, ComponentKind::Light));
    for id in [lamp, empty] {
        let light = scene.world.light(id).unwrap();
        assert_eq!(light.mode, LightMode::Baked);
        assert!(!light.cast_shadows && light.shadow_fade_distance == 0.0);
    }
}

/// A runtime light's fade follows its range while it is still the runtime
/// default; a fade the script set stays where the script put it.
#[test]
fn a_runtime_lights_fade_follows_its_range_until_a_script_sets_it() {
    let scene = Rc::new(RefCell::new(Scene::new()));
    let gun = scene.borrow_mut().add_entity("Gun".to_string());
    let source = r#"
        local step, flash = 0, nil
        return { Update = function()
            step = step + 1
            if step == 1 then
                flash = Scene.CreateEntity("Flash", "PointLight")
                Light.SetRange(flash, 2)
            elseif step == 2 then
                Light.SetShadowFadeDistance(flash, 30)
                Light.SetRange(flash, 5)
            end
        end }
    "#;
    attach_script(&mut scene.borrow_mut(), gun, "flash.lua", source);
    let mut game = playing(scene.clone(), 1.0);
    let fade = |s: &Rc<RefCell<Scene>>| {
        let s = s.borrow();
        let id = s.world.find_by_name("Flash").expect("the script made it");
        let light = s.world.light(id).expect("a light").clone();
        (light.range, light.shadow_fade_distance)
    };
    run(&mut game, 1);
    assert_eq!(
        fade(&scene),
        (2.0, 8.0),
        "four times the range it was set to"
    );
    run(&mut game, 1);
    assert_eq!(fade(&scene), (5.0, 30.0), "the script's fade stays");
}
