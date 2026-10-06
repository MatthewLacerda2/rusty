//! `Lighting.Generate` and the settings verbs (#832). The bakes themselves are
//! tested in `dev::generate_lighting`; here the bindings and their shapes.

use super::super::super::session::Session;

fn session() -> Session {
    Session::new("").expect("an empty session boots")
}

#[test]
fn settings_read_back_the_scenes_defaults() {
    let read = "local s = Lighting.GetSettings() \
        return table.concat({s.texelsPerUnit, s.samples, s.bounces, s.seed, tostring(s.directional)}, ' ')";
    assert_eq!(session().eval(read).unwrap(), "8.0 128 3 0 true");
}

#[test]
fn set_settings_changes_only_the_named_fields_and_clamps() {
    let s = session();
    s.eval("Lighting.SetSettings{ samples = 0, directional = false }")
        .unwrap();
    let settings = s.world().scene().borrow().lighting_settings.lightmaps;
    assert_eq!(settings.samples, 1, "clamped to one sample");
    assert!(!settings.directional);
    assert_eq!(settings.bounces, 3, "unnamed fields keep their value");
}

#[test]
fn generate_needs_a_saved_scene() {
    let err = session().eval("return Lighting.Generate()").unwrap_err();
    assert!(err.contains("save the scene"), "{err}");
}

#[test]
fn bake_lightmaps_takes_the_scenes_settings_for_missing_arguments() {
    use super::Overrides;
    let base = crate::scene::lighting::lightmap::BakeSettings {
        samples: 9,
        ..Default::default()
    };
    let merged = Overrides {
        bounces: Some(2),
        ..Default::default()
    }
    .apply(base);
    assert_eq!((merged.samples, merged.bounces), (9, 2));
}

#[test]
fn bake_lightmaps_applies_every_named_argument_and_clamps() {
    use super::Overrides;
    use crate::scene::lighting::lightmap::BakeSettings;
    let base = BakeSettings {
        directional: true,
        ..Default::default()
    };
    let named = Overrides {
        texels_per_unit: Some(3.0),
        samples: Some(7),
        bounces: Some(2),
        seed: Some(11),
        directional: Some(false),
    }
    .apply(base);
    let got = (named.texels_per_unit, named.samples, named.bounces);
    assert_eq!(got, (3.0, 7, 2));
    assert_eq!((named.seed, named.directional), (11, false));
    let clamped = Overrides {
        texels_per_unit: Some(0.0),
        samples: Some(0),
        bounces: Some(0),
        ..Default::default()
    }
    .apply(base);
    let got = (clamped.texels_per_unit, clamped.samples, clamped.bounces);
    assert_eq!(got, (0.01, 1, 1));
}
