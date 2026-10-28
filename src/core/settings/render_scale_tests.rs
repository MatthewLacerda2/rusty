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
fn round_trips_through_storage_and_defaults_to_native() {
    let mut storage = Storage::new();
    assert_eq!(RenderScale::load(&storage), RenderScale::NATIVE);
    RenderScale::new(0.75).store(&mut storage);
    assert_eq!(RenderScale::load(&storage), RenderScale::new(0.75));
    // A hand-edited value out of range comes back clamped.
    let far = serde_json::Value::from(9.0);
    storage.set(GRAPHICS_NAMESPACE, RENDER_SCALE_KEY, far);
    assert_eq!(RenderScale::load(&storage).get(), RenderScale::MAX);
}
