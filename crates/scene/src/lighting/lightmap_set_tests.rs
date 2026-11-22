use super::*;

#[test]
fn lookup_clear_and_round_trip() {
    let entry = |entity, page| LightmapEntry {
        entity,
        page,
        scale_offset: [0.5, 0.5, 0.0, 0.5],
    };
    let mut set = LightmapSet {
        pages: vec!["a.png".into(), "b.png".into()],
        directions: vec!["a_dir.png".into(), "b_dir.png".into()],
        ..LightmapSet::default()
    };
    set.extend([entry(5, 1), entry(3, 0)]);
    set.lights.insert(7);
    assert!(set.baked_light(7) && !set.baked_light(5));
    assert_eq!(set.get(5).map(|e| e.page), Some(1));
    assert!(set.get(9).is_none());
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
    assert!(
        json.find("\"entity\":3") < json.find("\"entity\":5"),
        "entity order"
    );
    let twice = r#"{"entries":[{"entity":3,"page":0,"scale_offset":[0,0,0,0]},
        {"entity":3,"page":1,"scale_offset":[0,0,0,0]}]}"#;
    let first = serde_json::from_str::<LightmapSet>(twice).unwrap();
    assert_eq!(first.get(3).map(|e| e.page), Some(0), "the first one wins");
    assert_eq!(set.remove(3).map(|e| e.page), Some(0));
    assert!(set.get(3).is_none());
    set.clear();
    assert!(set.is_empty() && set.pages.is_empty() && set.directions.is_empty());
    assert!(!set.baked_light(7), "a clear forgets the lights too");
}

#[test]
fn a_bake_records_the_mode_each_light_was_baked_as() {
    let mut set = LightmapSet::default();
    set.fell_back.insert(3);
    let taken = [
        (1, LightMode::Baked),
        (2, LightMode::Mixed),
        (3, LightMode::Mixed),
    ];
    set.record_taken(&taken.into_iter().collect());
    assert!(set.bakes_direct(1) && !set.bakes_direct(2) && set.bakes_direct(3));
    assert!(!set.bakes_direct(4), "never taken in");
    let json = serde_json::to_string(&set).unwrap();
    assert_eq!(serde_json::from_str::<LightmapSet>(&json).unwrap(), set);
    set.clear();
    assert!(set.is_empty() && !set.bakes_direct(1));
}
