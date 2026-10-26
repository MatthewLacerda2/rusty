//! The completeness gate's must-pass case (the engine as it is) and must-flag cases
//! (each part removed in turn).

use super::{check, engine};

#[test]
fn every_effect_has_every_part_in_every_layer() {
    let problems = check(&engine());
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn a_missing_part_is_flagged_by_name() {
    let mut l = engine();
    l.api.retain(|n| n != "bloom");
    l.sections.retain(|n| n != "vignette");
    l.docs = l.docs.replace("<!-- effect: fxaa -->", "");
    l.docs = l.docs.replace("`Graphics.bloom.threshold`", "");
    l.settings.push("lens_dirt".into());
    let problems = check(&l).join("\n");
    for needle in [
        "`bloom` has no Graphics.* registrar",
        "`vignette` has no Inspector section",
        "`fxaa` has no section in the Graphics doc",
        "`Graphics.bloom.threshold` is not in docs",
        "PostProcessingComponent lists `lens_dirt`",
    ] {
        assert!(
            problems.contains(needle),
            "not flagged: {needle}\n{problems}"
        );
    }
}

#[test]
fn a_camera_effect_needs_its_camera_field_and_an_option_its_name() {
    let mut l = engine();
    l.camera.retain(|f| !f.starts_with("motion_blur"));
    let vignette = l.render.iter_mut().find(|(n, ..)| n == "vignette").unwrap();
    vignette.2 = Some("bloom".into());
    let problems = check(&l).join("\n");
    assert!(
        problems.contains("`motion_blur` has no field on CameraComponent"),
        "{problems}"
    );
    assert!(
        problems.contains("`vignette` has no GraphicsOption named after it"),
        "{problems}"
    );
}

#[test]
fn a_volume_effect_needs_its_field_its_row_and_its_option_row() {
    let mut l = engine();
    l.settings.retain(|n| n != "bloom");
    l.ops.retain(|(n, _)| n != "vignette");
    l.docs = l.docs.replace("| `film_grain` |", "");
    let problems = check(&l).join("\n");
    for needle in [
        "`bloom` has no field on PostProcessingComponent",
        "`vignette` has no row in authoring's EFFECTS",
        "`film_grain` has no row in the docs' Player options table",
    ] {
        assert!(
            problems.contains(needle),
            "not flagged: {needle}\n{problems}"
        );
    }
}

#[test]
fn a_section_or_doc_marker_for_no_effect_is_flagged() {
    let mut l = engine();
    l.sections.push("lens_flare".into());
    l.docs.push_str("\n<!-- effect: lens_dirt -->\n");
    let problems = check(&l).join("\n");
    for needle in [
        "the Inspector's sections lists `lens_flare`",
        "docs/api/Graphics.md lists `lens_dirt`",
    ] {
        assert!(
            problems.contains(needle),
            "not flagged: {needle}\n{problems}"
        );
    }
}
