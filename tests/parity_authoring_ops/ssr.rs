//! `Graphics.*` SSR tracing bindings vs `authoring::rendering::post_processing::ssr`'s ops:
//! a script's write is what its `Get` reads back, and matches the shared op's.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::components::{CameraComponent, PostProcessingComponent, SsrTracing};
use rusty::edit::authoring::{default_post_processing, rendering::post_processing as vc_ops};
use rusty::scene::Scene;
use rusty::script::api::bind;

/// A camera with a volume under SSR's defaults, and its id.
fn scene() -> (Scene, u32) {
    let mut scene = Scene::new();
    let id = scene.add_entity("Camera".to_string());
    scene.world.set_camera(id, Some(CameraComponent::default()));
    scene
        .world
        .set_post_processing(id, Some(default_post_processing()));
    (scene, id)
}

const SCRIPT: &str = r#"
assert(Graphics.GetSsrTracing() == "RayMarching")
Graphics.SetSsrTracing("mixed")
assert(Graphics.GetSsrTracing() == "Mixed", Graphics.GetSsrTracing())
assert(not pcall(Graphics.SetSsrTracing, "pathtracing"))
assert(Graphics.GetSsrTracing() == "Mixed", "an unknown name changes nothing")
Graphics.SetSsrTraceResolutionDivisor(1)
assert(Graphics.GetSsrTraceResolutionDivisor() == 1)
Graphics.SetSsrTraceResolutionDivisor(0)
assert(Graphics.GetSsrTraceResolutionDivisor() == 1, "clamped up to 1")
Graphics.SetSsrTraceResolutionDivisor(99)
"#;

fn apply_ops(vc: &mut PostProcessingComponent) {
    vc_ops::ssr::set_tracing(vc, SsrTracing::Mixed);
    vc_ops::ssr::set_trace_resolution_divisor(vc, 99);
}

#[test]
fn ssr_tracing_setters_change_what_their_getters_read_and_match_the_shared_op() {
    let (lua_scene, id) = scene();
    let lua_scene = Rc::new(RefCell::new(lua_scene));
    let (graphics, app) = (Rc::default(), Rc::default());
    let lua = Lua::new();
    crate::lua::attempt(|| {
        rusty::script::api::graphics::register(
            &lua,
            bind(&lua, &lua_scene),
            (
                bind(&lua, &graphics),
                rusty::script::api::ApiCell::default(),
            ),
            bind(&lua, &app),
        )
        .unwrap();
        lua.load(SCRIPT).exec()
    })
    .unwrap();
    let (mut op_scene, op_id) = scene();
    apply_ops(&mut op_scene.world.post_processing_mut(op_id).unwrap());
    let ssr = |s: &Scene, id| s.world.post_processing(id).unwrap().ssr.clone();
    let got = ssr(&lua_scene.borrow(), id);
    assert_eq!(format!("{got:?}"), format!("{:?}", ssr(&op_scene, op_id)));
    assert_eq!(got.tracing, SsrTracing::Mixed);
    let cap = rusty::core::settings::quality::MAX_RESOLUTION_DIVISOR;
    assert_eq!(got.trace_resolution_divisor, cap, "clamped down to the cap");
}
