//! The card's slider rows show a value outside their range without rewriting it: a
//! compensation a script set past the slider's reach survives the camera being
//! selected (#1444).

use crate::components::AutoExposure;
use crate::ecs::World;
use crate::scene::authoring::default_post_processing;
use crate::scene::authoring::rendering::post_processing as vc_ops;

/// Draw one compensation row over a volume holding `ev`; return whether the row
/// reported a change, and the value the volume holds after.
fn drawn(ev: f32) -> (bool, f32) {
    let mut world = World::new();
    let id = world.spawn("Camera".to_string());
    let mut vc = default_post_processing();
    vc.exposure.automatic.compensation = ev;
    world.set_post_processing(id, Some(vc));
    let ctx = egui::Context::default();
    let mut changed = false;
    let mut out = ctx.run_ui(Default::default(), |ui| {
        let r = AutoExposure::EV_RANGE;
        changed = super::post_processing::slider_row(
            ui,
            (&mut world, id),
            "Compensation:",
            ev,
            -r / 2.0..=r / 2.0,
            vc_ops::set_exposure_compensation,
        );
    });
    out.textures_delta.clear();
    let after = world
        .post_processing(id)
        .unwrap()
        .exposure
        .automatic
        .compensation;
    (changed, after)
}

#[test]
fn a_value_past_the_slider_is_shown_not_clamped() {
    assert_eq!(drawn(8.0), (false, 8.0));
    assert_eq!(drawn(-9.5), (false, -9.5));
    assert_eq!(drawn(1.0), (false, 1.0));
}
