//! A camera's FOV axis (#2038, Unity's *FOV Axis*): display only, the stored fov
//! stays vertical; Unity's two conversions; the `Camera.*` round-trip and error.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::Lua;
use rusty::api::bind;
use rusty::components::{
    horizontal_to_vertical_fov, vertical_to_horizontal_fov, CameraComponent, FovAxis,
};
use rusty::scene::authoring::rendering::camera as camera_ops;
use rusty::scene::Scene;

const WIDE: f32 = 16.0 / 9.0;

fn run(scene: &Rc<RefCell<Scene>>, code: &str) -> mlua::Result<()> {
    let lua = Lua::new();
    crate::lua::attempt(|| {
        let table = lua.create_table()?;
        rusty::api::camera::component::register(&lua, &table, bind(&lua, scene)).unwrap();
        lua.globals().set("Camera", table)?;
        lua.load(code).exec()
    })
}

fn scene_with_camera() -> (Rc<RefCell<Scene>>, u32) {
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".to_string());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    (Rc::new(RefCell::new(scene)), cam)
}

#[test]
fn ninety_horizontal_at_sixteen_nine_is_the_issues_vertical() {
    let v = horizontal_to_vertical_fov(90.0, WIDE);
    assert!((v - 58.7155).abs() < 1e-3, "{v}");
    assert!((vertical_to_horizontal_fov(v, WIDE) - 90.0).abs() < 1e-3);
    // The prefab's 85 vertical is about 117 horizontal at 16:9.
    let h = vertical_to_horizontal_fov(85.0, WIDE);
    assert!((h - 116.9).abs() < 0.1, "{h}");
    // A square view has one angle on both axes.
    assert!((vertical_to_horizontal_fov(60.0, 1.0) - 60.0).abs() < 1e-4);
}

#[test]
fn flipping_the_axis_leaves_the_stored_fov() {
    let mut c = CameraComponent::default();
    camera_ops::set_fov(&mut c, 58.7);
    camera_ops::set_fov_axis(&mut c, FovAxis::Horizontal);
    assert_eq!((c.fov, c.fov_axis), (58.7, FovAxis::Horizontal));
    camera_ops::set_fov_axis(&mut c, FovAxis::Vertical);
    assert_eq!(c.fov, 58.7);
    // An edit along the horizontal axis stores the vertical angle, held 1..179.
    camera_ops::set_fov_along(&mut c, FovAxis::Horizontal, 90.0, WIDE);
    assert!((c.fov - 58.7155).abs() < 1e-3, "{}", c.fov);
    camera_ops::set_fov_along(&mut c, FovAxis::Horizontal, f32::NAN, WIDE);
    assert_eq!(c.fov, camera_ops::MIN_FOV);
}

#[test]
fn a_scene_camera_without_the_field_loads_vertical() {
    let mut value = serde_json::to_value(CameraComponent::default()).unwrap();
    value.as_object_mut().unwrap().remove("fov_axis");
    let c: CameraComponent = serde_json::from_value(value).unwrap();
    assert_eq!(c.fov_axis, FovAxis::Vertical);
}

#[test]
fn the_axis_round_trips_through_lua_and_never_moves_the_fov() {
    let (scene, cam) = scene_with_camera();
    let code = format!(
        r#"
        assert(Camera.GetFovAxis({cam}) == "Vertical")
        Camera.SetFieldOfView({cam}, 60)
        Camera.SetFovAxis({cam}, "horizontal")
        assert(Camera.GetFovAxis({cam}) == "Horizontal")
        assert(Camera.GetFieldOfView({cam}) == 60)
        local v = Camera.HorizontalToVerticalFov(90, 16 / 9)
        assert(math.abs(v - 58.7155) < 1e-3, v)
        assert(math.abs(Camera.VerticalToHorizontalFov(v, 16 / 9) - 90) < 1e-3)
    "#
    );
    run(&scene, &code).unwrap();
    let s = scene.borrow();
    let c = s.world.camera(cam).unwrap();
    assert_eq!((c.fov, c.fov_axis), (60.0, FovAxis::Horizontal));
}

#[test]
fn an_unknown_axis_is_an_error_naming_the_choices() {
    let (scene, cam) = scene_with_camera();
    let err = run(&scene, &format!("Camera.SetFovAxis({cam}, \"Diagonal\")")).unwrap_err();
    let text = err.to_string();
    assert!(
        text.contains("Vertical") && text.contains("Horizontal"),
        "{text}"
    );
    assert_eq!(
        scene.borrow().world.camera(cam).unwrap().fov_axis,
        FovAxis::Vertical
    );
}
