//! Widgets on a world canvas (#429): the pointer's camera ray reaches them, and
//! `event.canvas_position` maps it onto the widget — a slider is set by looking at
//! it and clicking with the cursor locked, and a dropdown opens on its own canvas.

use glam::Vec3;
use rusty::components::CanvasRenderMode;
use rusty::scene::authoring::ui_widgets::UiWidget;
use rusty::scene::Scene;

use super::{part, widget, Ui};

/// Make `id`'s root canvas a `WorldSpace` canvas 10 m in front of the origin.
fn to_world(scene: &mut Scene, id: u32) -> u32 {
    let mut canvas = id;
    while let Some(p) = scene.world.parent_id(canvas) {
        canvas = p;
    }
    scene.world.canvas_mut(canvas).expect("canvas").render_mode = CanvasRenderMode::WorldSpace;
    scene.world.transform_mut(canvas).expect("t").position = Vec3::new(0.0, 0.0, -10.0);
    canvas
}

/// Aim the camera (looking down -Z) at canvas point `(x, y)` of a default
/// 1920×1080, 100-units-per-metre world canvas 10 m ahead.
fn look_at(ui: &Ui, (x, y): (f32, f32)) {
    let (wx, wy) = ((x - 960.0) / 100.0, (y - 540.0) / 100.0);
    ui.eval(&format!(
        "Camera.SetPosition({wx}, {wy}, 0) Camera.SetYaw(-90) Camera.SetPitch(0)"
    ));
}

/// `id`'s rect in canvas units: `(x, y, width, height)`.
fn canvas_rect(ui: &Ui, id: u32) -> (f32, f32, f32, f32) {
    let s = ui.eval(&format!(
        "local r = UI.GetRect({id}) return r.x, r.y, r.width, r.height"
    ));
    let v: Vec<f32> = s.split(", ").map(|n| n.parse().expect("number")).collect();
    (v[0], v[1], v[2], v[3])
}

#[test]
fn a_locked_look_and_click_sets_a_world_slider() {
    let (mut sl, mut area) = (0, 0);
    let mut ui = Ui::new(|s| {
        sl = widget(s, UiWidget::Slider);
        area = part(s, sl, "Handle Slide Area");
        to_world(s, sl);
    });
    let (x, y, w, h) = canvas_rect(&ui, area);
    look_at(&ui, (x + w * 0.75, y + h * 0.5));
    ui.eval("Input.SetCursorLocked(true)");
    ui.eval("Input.Press('Mouse0')");
    ui.tick(1);
    ui.eval("Input.Release('Mouse0')");
    ui.tick(1);
    let v: f32 = ui
        .eval(&format!("Scene.GetScript({sl}, 'slider').value"))
        .parse()
        .expect("number");
    assert!((v - 0.75).abs() < 0.01, "value {v}: {:?}", ui.errors());
}

#[test]
fn a_world_dropdown_opens_on_its_own_canvas() {
    let (mut dd, mut canvas) = (0, 0);
    let mut ui = Ui::new(|s| {
        dd = widget(s, UiWidget::Dropdown);
        canvas = to_world(s, dd);
    });
    let (x, y, w, h) = canvas_rect(&ui, dd);
    look_at(&ui, (x + w * 0.5, y + h * 0.5));
    ui.eval("Input.SetCursorLocked(true)");
    ui.click(dd);
    assert_eq!(
        ui.eval(&format!(
            "return Scene.GetScript({dd}, 'dropdown').is_expanded()"
        )),
        "true"
    );
    let template = ui.eval(&format!("return Scene.FindChild({dd}, 'Template')"));
    assert_eq!(template, "nil", "the list moved out of the dropdown");
    let popup = ui.eval(&format!(
        "return UI.GetRect(Scene.GetChildren({canvas})[#Scene.GetChildren({canvas})]).canvas"
    ));
    assert_eq!(
        popup,
        canvas.to_string(),
        "…onto the dropdown's own world canvas"
    );
}
