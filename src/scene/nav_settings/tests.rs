//! Tests for the per-scene navmesh settings: defaults, back-compat and the saved shape.

use super::*;

/// The struct `Default` and every field's serde default must equal the historical
/// bake constants, so an older scene missing the block bakes exactly as before.
#[test]
fn defaults_equal_bake_constants() {
    let d = NavMeshSettings::default();
    assert_eq!(d.max_step, DEFAULT_MAX_STEP);
    assert_eq!(d.max_slope, DEFAULT_MAX_SLOPE);
    assert_eq!(d.grid_spacing, DEFAULT_GRID_SPACING);
    assert_eq!(d.agent_radius, DEFAULT_AGENT_RADIUS);
    assert_eq!(d.agent_height, DEFAULT_AGENT_HEIGHT);
    assert_eq!(d.bounds, None, "bounds derive from the scene by default");
    assert_eq!(d.areas, default_areas());
}

/// The area table never forces a rebake: costs are read at search time (#460).
#[test]
fn the_area_table_is_not_a_bake_input() {
    let mut a = NavMeshSettings::default();
    a.areas[0].cost = 5.0;
    assert!(a.bakes_like(&NavMeshSettings::default()));
    a.max_step = 0.3;
    assert!(!a.bakes_like(&NavMeshSettings::default()));
}

/// A JSON object missing every field deserializes to the defaults (back-compat).
#[test]
fn empty_json_object_fills_defaults() {
    let s: NavMeshSettings = serde_json::from_str("{}").expect("empty object loads");
    assert_eq!(s, NavMeshSettings::default());
}

/// A pre-#278 scene that carries the other knobs but omits `agent_height` still loads,
/// filling the new field with its serde default (back-compat for the height addition).
#[test]
fn legacy_json_without_agent_height_uses_default() {
    let json = r#"{"agent_radius":0.5,"max_slope":1.0,"max_step":0.5,"grid_spacing":1.0}"#;
    let s: NavMeshSettings = serde_json::from_str(json).expect("legacy object loads");
    assert_eq!(s.agent_height, DEFAULT_AGENT_HEIGHT);
    assert_eq!(s, NavMeshSettings::default());
}

#[test]
fn round_trips_through_json() {
    let s = NavMeshSettings {
        agent_radius: 0.7,
        agent_height: 1.8,
        max_slope: 0.5,
        max_step: 0.25,
        grid_spacing: 2.0,
        bounds: Some(NavBounds {
            min_x: -60.0,
            max_x: 80.0,
            min_z: -5.0,
            max_z: 5.0,
        }),
        drop_height: 4.0,
        jump_distance: 1.5,
        jump_height: 1.2,
        link_spacing: 3.0,
        areas: default_areas(),
    };
    let json = serde_json::to_string(&s).expect("serialize");
    let back: NavMeshSettings = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(s, back);
}

/// The saved shape is pinned byte for byte (#721 moved the type out of `navigation`):
/// same field names, same order, same serde defaults, so every scene file on disk
/// loads and re-saves unchanged.
const SAVED_DEFAULT: &str = concat!(
    r#"{"agent_radius":0.5,"agent_height":2.0,"max_slope":1.0,"max_step":0.5,"#,
    r#""grid_spacing":1.0,"bounds":null,"drop_height":0.0,"jump_distance":0.0,"#,
    r#""jump_height":0.0,"link_spacing":2.0,"areas":[{"name":"Walkable","cost":1.0},"#,
    r#"{"name":"NotWalkable","cost":1.0}]}"#
);

#[test]
fn the_saved_shape_is_byte_stable() {
    let json = serde_json::to_string(&NavMeshSettings::default()).expect("serialize");
    assert_eq!(json, SAVED_DEFAULT);
    let back: NavMeshSettings = serde_json::from_str(SAVED_DEFAULT).expect("loads");
    assert_eq!(back, NavMeshSettings::default());
}

#[test]
fn a_saved_bounds_override_keeps_its_shape() {
    let json = r#"{"min_x":-60.0,"max_x":80.0,"min_z":-5.0,"max_z":5.0}"#;
    let b = NavBounds::new(-60.0, 80.0, -5.0, 5.0);
    assert_eq!(serde_json::to_string(&b).expect("serialize"), json);
    assert_eq!(serde_json::from_str::<NavBounds>(json).expect("loads"), b);
}
