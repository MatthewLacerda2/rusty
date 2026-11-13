use super::*;

#[test]
fn new_clamps_to_urps_range_and_maps_nan_to_native() {
    assert_eq!(RenderScale::new(0.0).get(), RenderScale::MIN);
    assert_eq!(RenderScale::new(5.0).get(), RenderScale::MAX);
    assert_eq!(RenderScale::new(0.5).get(), 0.5);
    assert_eq!(RenderScale::new(f32::NAN), RenderScale::NATIVE);
    assert_eq!(RenderScale::default(), RenderScale::NATIVE);
}

#[test]
fn apply_rounds_and_never_reaches_zero() {
    assert_eq!(RenderScale::new(0.5).apply(2940), 1470);
    assert_eq!(RenderScale::new(0.5).apply(1389), 695);
    assert_eq!(RenderScale::NATIVE.apply(1388), 1388);
    assert_eq!(RenderScale::new(2.0).apply(640), 1280);
    assert_eq!(RenderScale::new(0.1).apply(4), 1);
}

#[test]
fn round_trips_through_storage_and_none_is_no_choice() {
    let mut storage = Storage::new();
    assert_eq!(RenderScale::load(&storage), None);
    RenderScale::new(0.75).store(&mut storage);
    assert_eq!(RenderScale::load(&storage), Some(RenderScale::new(0.75)));
    // A hand-edited value out of range comes back clamped.
    let far = serde_json::Value::from(9.0);
    storage.set(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY, far);
    assert_eq!(
        RenderScale::load(&storage).map(RenderScale::get),
        Some(RenderScale::MAX)
    );
}

#[test]
fn the_project_default_runs_while_storage_holds_no_choice() {
    let half = RenderScaleDefault::Fixed(RenderScale::new(0.5));
    let mut storage = Storage::new();
    assert_eq!(
        RenderScaleSetting::load(&storage, half).resolve(1.0),
        RenderScale::new(0.5)
    );
    // A stored player value overrides it.
    RenderScale::new(0.8).store(&mut storage);
    let setting = RenderScaleSetting::load(&storage, half);
    assert_eq!(setting.resolve(1.0), RenderScale::new(0.8));
    // Forgetting the choice deletes the key, so the default is back next launch.
    let forgot = RenderScaleSetting {
        chosen: None,
        ..setting
    };
    forgot.store(&mut storage);
    assert!(storage.get(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY).is_none());
    assert_eq!(
        RenderScaleSetting::load(&storage, half).resolve(2.0),
        RenderScale::new(0.5)
    );
}

#[test]
fn match_display_is_one_over_the_display_scale() {
    let matching = RenderScaleDefault::MatchDisplay;
    assert_eq!(matching.resolve(1.0), RenderScale::NATIVE);
    assert_eq!(matching.resolve(2.0), RenderScale::new(0.5));
    // A monitor change is a new display scale: the next resolve follows it.
    let setting = RenderScaleSetting {
        chosen: None,
        default: matching,
    };
    assert_eq!(setting.resolve(2.0).get(), 0.5);
    assert_eq!(setting.resolve(1.0).get(), 1.0);
    for bad in [0.0, -2.0, f32::NAN] {
        assert_eq!(matching.resolve(bad), RenderScale::NATIVE, "{bad}");
    }
}

#[test]
fn the_graphics_block_reads_a_factor_or_match_display() {
    let read = |json: &str| ProjectGraphics::read_json(&serde_json::from_str(json).unwrap());
    assert_eq!(read("{}"), (ProjectGraphics::default(), vec![]));
    let fixed = RenderScaleDefault::Fixed(RenderScale::new(0.75));
    assert_eq!(read(r#"{"render_scale": 0.75}"#).0.render_scale, fixed);
    let matching = read(r#"{"render_scale": "match_display"}"#).0;
    assert_eq!(matching.render_scale, RenderScaleDefault::MatchDisplay);
    // Out of range clamps, as the player's value does.
    let far = read(r#"{"render_scale": 9}"#).0.render_scale;
    assert_eq!(
        far,
        RenderScaleDefault::Fixed(RenderScale::new(RenderScale::MAX))
    );
    for bad in ["0", "-1", r#""half""#, "null", "true"] {
        let (graphics, ignored) = read(&format!(r#"{{"render_scale": {bad}}}"#));
        assert_eq!(graphics, ProjectGraphics::default(), "{bad} keeps 1");
        assert_eq!(ignored.len(), 1, "{bad} is warned about");
    }
    assert_eq!(
        read(r#"{"render": 1}"#).1.len(),
        1,
        "a typo is warned about"
    );
    assert_eq!(read("[]").1.len(), 1, "not an object is warned about");
}

#[test]
fn the_graphics_block_round_trips_through_the_project_file() {
    use crate::project::ProjectFile;
    for default in [
        RenderScaleDefault::MatchDisplay,
        RenderScaleDefault::Fixed(RenderScale::new(0.5)),
    ] {
        let mut file = ProjectFile::default();
        file.graphics.render_scale = default;
        let back = ProjectFile::from_json(&file.to_json()).unwrap();
        assert_eq!(back.graphics.render_scale, default);
    }
    // The factor is written as the `f32` it is: a hand-edited file reads `0.7`.
    let mut file = ProjectFile::default();
    file.graphics.render_scale = RenderScaleDefault::Fixed(RenderScale::new(0.7));
    let text = file.to_json();
    assert!(text.contains(r#""render_scale": 0.7"#), "{text}");
    // A bad value still opens the project, on the default.
    let file = ProjectFile::from_json(r#"{"graphics": {"render_scale": "big"}}"#).unwrap();
    assert_eq!(file.graphics, ProjectGraphics::default());
}
