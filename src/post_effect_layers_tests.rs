//! The post effects' completeness gate (`render::postfx::completeness`) over every
//! layer, the two above `render` included: each effect has its `Graphics.*`
//! registrar (`script::api::graphics::effects::REGISTRARS`) and its Inspector section, and
//! neither lists an effect the renderer lacks. The gate's own cases are `render`'s
//! tests.

use crate::render::postfx::completeness::problems;

/// The Inspector's sections, by the effect each draws (both cards).
#[cfg(feature = "editor")]
fn sections() -> Vec<&'static str> {
    use crate::editor::inspector::components::camera::{post_processing, CAMERA_SECTIONS};
    let volume = post_processing::SECTIONS.iter().map(|s| s.effect);
    volume
        .chain(CAMERA_SECTIONS.iter().map(|s| s.effect))
        .collect()
}

#[test]
fn every_effect_has_every_part_in_every_layer() {
    let api: Vec<&str> = crate::script::api::graphics::effects::REGISTRARS
        .iter()
        .map(|(n, _)| *n)
        .collect();
    // Without the editor there is no card to check: a registered effect counts as
    // drawn, so only the registrars are checked.
    #[cfg(not(feature = "editor"))]
    let sections = api.clone();
    #[cfg(feature = "editor")]
    let sections = sections();
    let problems = problems(&api, &sections);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
