//! `Graphics.*` depth-of-field bindings vs `authoring::rendering::post_processing`'s
//! depth-of-field ops (#1445): the same writes through both reach the same component.

use std::cell::RefCell;

use mlua::Lua;
use rusty::components::{CameraComponent, DofMode, PostProcessingComponent};
use rusty::scene::authoring::{default_post_processing, rendering::post_processing as vc_ops};
use rusty::scene::Scene;

/// A camera with an active volume, and its id.
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
Graphics.SetDepthOfFieldMode("bokeh")
Graphics.SetDepthOfFieldGaussianStart(50)
Graphics.SetDepthOfFieldGaussianEnd(20)
Graphics.SetDepthOfFieldGaussianMaxRadius(9)
Graphics.SetDepthOfFieldFocusDistance(-1)
Graphics.SetDepthOfFieldAperture(1.8)
Graphics.SetDepthOfFieldFocalLength(500)
"#;

fn apply_ops(vc: &mut PostProcessingComponent) {
    vc_ops::set_dof_mode(vc, DofMode::Bokeh);
    vc_ops::set_dof_gaussian_start(vc, 50.0);
    vc_ops::set_dof_gaussian_end(vc, 20.0);
    vc_ops::set_dof_gaussian_max_radius(vc, 9.0);
    vc_ops::set_dof_focus_distance(vc, -1.0);
    vc_ops::set_dof_aperture(vc, 1.8);
    vc_ops::set_dof_focal_length(vc, 500.0);
}

#[test]
fn depth_of_field_api_and_shared_op_converge() {
    let (lua_scene, id) = scene();
    let lua_scene = RefCell::new(lua_scene);
    let (graphics, app) = (RefCell::default(), RefCell::default());
    let lua = Lua::new();
    lua.scope(|s| {
        rusty::api::graphics::register(&lua, s, &lua_scene, &graphics, &app).unwrap();
        lua.load(SCRIPT).exec()
    })
    .unwrap();
    let (mut op_scene, op_id) = scene();
    apply_ops(&mut op_scene.world.post_processing_mut(op_id).unwrap());
    let dof = |s: &Scene, id| s.world.post_processing(id).unwrap().depth_of_field;
    let got = dof(&lua_scene.borrow(), id);
    assert_eq!(got, dof(&op_scene, op_id));
    assert_eq!(got.mode, DofMode::Bokeh, "names are case-insensitive");
    assert_eq!(
        got.gaussian_end, 20.0,
        "the end is independent of the start"
    );
    assert_eq!((got.focus_distance, got.focal_length), (0.1, 300.0));
}
