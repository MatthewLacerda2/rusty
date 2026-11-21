//! A light a script makes while the game plays casts a shadow (#2522): it is
//! `Realtime` with `cast_shadows` on, so the frame's shadow atlas gives it tiles.
//! The default scene holds no point or spot light of its own, so its atlas starts
//! empty.

use rusty::dev::harness::Harness;
use rusty::edit::default_scene::layout::PLAYER_SPAWN;

use crate::play::attach_script;

#[test]
fn a_lamp_a_script_makes_in_update_gets_shadow_tiles() {
    let out = crate::temp::dir().join(format!("rusty_runtime_shadow_{}", std::process::id()));
    let mut h = Harness::new(&out, "");
    let [x, y, z] = PLAYER_SPAWN;
    let source = format!(
        r#"
        local done = false
        return {{ Update = function()
            if done then return end
            done = true
            local lamp = Scene.CreateEntity("Flash", "PointLight")
            Transform.SetPosition(lamp, {x}, {y} + 1.5, {z} + 3)
        end }}
    "#
    );
    {
        let world = h.world.borrow();
        let mut scene = world.scene().borrow_mut();
        let host = scene.add_entity("Gun".to_string());
        attach_script(&mut scene, host, "runtime_flash.lua", &source);
    }
    h.step(3);
    if !h.screenshot(out.join("shot.png")) {
        assert!(
            !rusty::render::gpu_required(),
            "RUSTY_REQUIRE_GPU=1 but no shot"
        );
        eprintln!("[runtime shadow] no GPU/software adapter — skipping");
        return;
    }
    let flash = {
        let world = h.world.borrow();
        let scene = world.scene().borrow();
        let id = scene.world.find_by_name("Flash").expect("the script ran");
        let light = scene.world.light(id).expect("a light").clone();
        light
    };
    assert!(flash.cast_shadows && flash.shadow_fade_distance > flash.range);
    let shadowed = h
        .stats
        .borrow()
        .get("shadowed_lights")
        .map_or(0.0, |s| s.max);
    assert_eq!(
        shadowed, 1.0,
        "the runtime lamp holds the atlas's one shadow"
    );
    let _ = std::fs::remove_dir_all(out);
}
