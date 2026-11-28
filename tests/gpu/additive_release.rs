//! An additive scene unloaded where its members' destroys run a tick later (the console's
//! command channel, between ticks; a script's `OnDestroy`) still has its GPU meshes released,
//! on the first frame drawn after its entities have left the World.

use rusty::dev::harness::Harness;
use rusty::scene::create::{create_entity, Primitive};
use rusty::scene::{Scene, ScriptComponent};

/// A base scene (a box, and `Trigger`, whose `OnDestroy` unloads the piece) and the piece (a
/// sphere under a root), saved in `dir`.
fn files(dir: &std::path::Path) -> (String, String) {
    std::fs::create_dir_all(dir).unwrap();
    let file = |name: &str| rusty::core::paths::to_slash(&dir.join(name));
    let (base, piece, lua) = (file("base.scene"), file("piece.scene"), file("trigger.lua"));
    let code = format!("return {{ OnDestroy = function() Scene.Unload('{piece}') end }}");
    std::fs::write(&lua, code).unwrap();
    let mut scene = Scene::new();
    create_entity(&mut scene, "Floor", Some(Primitive::Box));
    let trigger = scene.add_entity("Trigger".into());
    *scene.world.scripts_mut(trigger).unwrap() = vec![ScriptComponent {
        path: lua,
        ..Default::default()
    }];
    rusty::edit::save_to_file(&scene, &base).unwrap();
    let mut scene = Scene::new();
    let root = scene.add_entity("Piece".into());
    let ball = create_entity(&mut scene, "Ball", Some(Primitive::Sphere));
    scene.set_parent(ball, Some(root)).unwrap();
    rusty::edit::save_to_file(&scene, &piece).unwrap();
    (base, piece)
}

fn meshes(h: &mut Harness) -> Option<u32> {
    let frame = h.render_frame();
    if frame.is_none() {
        assert!(
            !rusty::render::gpu_required(),
            "RUSTY_REQUIRE_GPU=1 but no frame"
        );
    }
    Some(frame?.counters.mesh_count)
}

fn eval(h: &Harness, line: &str) {
    h.world.borrow().script_manager().eval(line).unwrap();
}

/// The piece loaded additively and drawn: the frame's mesh count, and the base's alone.
fn loaded(h: &mut Harness, base: &str, piece: &str) -> Option<(u32, u32)> {
    eval(h, &format!("Scene.Load('{base}')"));
    h.step(2);
    let alone = meshes(h)?;
    eval(h, &format!("Scene.Load('{piece}', 'Additive')"));
    h.step(2);
    let with = meshes(h)?;
    assert!(
        with > alone,
        "the piece's sphere is resident: {with} vs {alone}"
    );
    Some((alone, with))
}

#[test]
fn an_unload_from_the_command_channel_releases_once_its_members_are_gone() {
    let dir = crate::temp::dir().join("additive_release_console");
    let (base, piece) = files(&dir);
    let mut h = Harness::new(dir.join("run"), "");
    h.step(1);
    let Some((alone, with)) = loaded(&mut h, &base, &piece) else {
        return;
    };
    eval(&h, &format!("Scene.Unload('{piece}')"));
    assert_eq!(
        meshes(&mut h),
        Some(with),
        "drawn before the tick: still a member"
    );
    h.step(1);
    assert_eq!(
        meshes(&mut h),
        Some(alone),
        "the frame after they left releases"
    );
}

#[test]
fn an_unload_from_on_destroy_releases_once_its_members_are_gone() {
    let dir = crate::temp::dir().join("additive_release_on_destroy");
    let (base, piece) = files(&dir);
    let mut h = Harness::new(dir.join("run"), "");
    h.step(1);
    let Some((alone, with)) = loaded(&mut h, &base, &piece) else {
        return;
    };
    eval(&h, "Scene.DestroyEntity(Scene.FindEntityByName('Trigger'))");
    h.step(1);
    let ball_left = h_find(&h, "Ball");
    let drawn = meshes(&mut h);
    if ball_left {
        assert_eq!(drawn, Some(with), "the piece is still in the World");
    }
    h.step(1);
    assert!(!h_find(&h, "Ball"), "the piece's destroys ran");
    assert_eq!(
        meshes(&mut h),
        Some(alone),
        "the frame after they left releases"
    );
}

fn h_find(h: &Harness, name: &str) -> bool {
    let world = h.world.borrow();
    let found = world.scene().borrow().find_entity_by_name(name);
    found.is_some()
}
