//! src/scene/authoring/ui_widgets/range.rs — `Slider` and `Scrollbar`.
//!
//! Both are a track with a handle the script places by anchors: the slider's
//! `Fill Area/Fill` stretches from the start to the value and its
//! `Handle Slide Area/Handle` sits at it; the scrollbar's `Sliding Area/Handle`
//! spans `size` of the track at `value`. Laid out horizontally; the script
//! reorients them for a vertical `direction`.

use glam::{Vec2, Vec4};

use super::parts::{image, node, script, script_with, selectable, Place, WHITE};
use crate::components::ScriptFieldValue;
use crate::scene::Scene;

const TRACK: Vec4 = Vec4::new(0.8, 0.8, 0.8, 1.0);
const FILL: Vec4 = Vec4::new(0.55, 0.55, 0.55, 1.0);

/// `Slider` — background, fill, handle, `slider.lua`.
pub fn slider(scene: &mut Scene, parent: Option<u32>) -> u32 {
    let id = node(
        scene,
        "Slider",
        parent,
        Place::centred(Vec2::new(320.0, 40.0)),
    );
    let band = |lo: f32, hi: f32| (Vec2::new(0.0, lo), Vec2::new(1.0, hi));
    let (lo, hi) = band(0.25, 0.75);
    let bg = node(
        scene,
        "Background",
        Some(id),
        Place::stretch(lo, hi, Vec2::ZERO),
    );
    image(scene, bg, TRACK);
    let area = node(
        scene,
        "Fill Area",
        Some(id),
        Place::stretch(lo, hi, Vec2::new(-20.0, 0.0)),
    );
    let fill = node(scene, "Fill", Some(area), Place::fill(Vec2::ZERO));
    image(scene, fill, FILL);
    let slide = node(
        scene,
        "Handle Slide Area",
        Some(id),
        Place::fill(Vec2::new(10.0, 0.0)),
    );
    let handle = node(
        scene,
        "Handle",
        Some(slide),
        Place::stretch(Vec2::ZERO, Vec2::new(0.0, 1.0), Vec2::new(20.0, 0.0)),
    );
    image(scene, handle, WHITE);
    selectable(scene, id, Some(handle));
    script(scene, id, "slider");
    id
}

/// `Scrollbar` — a track with a handle spanning `size`, `scrollbar.lua`.
pub fn scrollbar(scene: &mut Scene, parent: Option<u32>) -> u32 {
    scrollbar_sized(scene, parent, Place::centred(Vec2::new(320.0, 40.0)), None)
}

/// A scrollbar at `place`; `direction` overrides the script's default
/// (`"LeftToRight"`), e.g. `"BottomToTop"` for a scroll view's vertical bar.
pub fn scrollbar_sized(
    scene: &mut Scene,
    parent: Option<u32>,
    place: Place,
    direction: Option<&str>,
) -> u32 {
    let id = node(scene, "Scrollbar", parent, place);
    image(scene, id, TRACK);
    let area = node(
        scene,
        "Sliding Area",
        Some(id),
        Place::fill(Vec2::splat(4.0)),
    );
    let handle = node(scene, "Handle", Some(area), Place::fill(Vec2::ZERO));
    image(scene, handle, WHITE);
    selectable(scene, id, Some(handle));
    match direction {
        Some(d) => script_with(
            scene,
            id,
            "scrollbar",
            &[("direction", ScriptFieldValue::Text(d.to_string()))],
        ),
        None => script(scene, id, "scrollbar"),
    }
    id
}
