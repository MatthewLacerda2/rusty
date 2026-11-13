//! Slots and montages on the graph (#1894): they parse with Unreal's defaults,
//! and every bad one is reported under its path.

use super::{AnimationGraph, DEFAULT_SLOT};

fn parsed(extra: &str) -> AnimationGraph {
    let json =
        format!(r#"{{ "nodes": [{{ "name": "Run", "clip": "Run" }}], "entry": "Run", {extra} }}"#);
    serde_json::from_str(&json).unwrap()
}

fn problems(g: &AnimationGraph) -> Vec<String> {
    match g.validate() {
        Ok(()) => Vec::new(),
        Err(super::GraphError::Invalid(p)) => p,
        Err(e) => panic!("{e}"),
    }
}

#[test]
fn montages_parse_with_defaults_and_resolve_their_slots() {
    let g = parsed(
        r#""slots": [{ "name": "UpperBody", "mask": ["spine_01"] }],
           "montages": [{ "name": "Flinch", "clip": "Hit" },
                        { "name": "Reload", "clip": "R", "slot": "UpperBody", "blend_in": 0.15 }]"#,
    );
    assert_eq!(problems(&g), Vec::<String>::new());
    let flinch = g.montage("Flinch").unwrap();
    assert_eq!(flinch.slot, DEFAULT_SLOT);
    assert_eq!((flinch.blend_in, flinch.blend_out), (0.25, 0.25));
    assert_eq!(g.slot("UpperBody").unwrap().0, 0);
    // The implicit full-body slot sits above the declared ones.
    let (order, slot) = g.slot(DEFAULT_SLOT).unwrap();
    assert_eq!((order, slot.mask.len()), (1, 0));
    assert!(g.slot("Legs").is_none());
}

#[test]
fn a_graph_without_montages_saves_without_the_fields() {
    let json = serde_json::to_string(&parsed(r#""layers": []"#)).unwrap();
    assert!(
        !json.contains("slots") && !json.contains("montages"),
        "{json}"
    );
}

#[test]
fn every_bad_slot_and_montage_names_its_path() {
    let g = parsed(
        r#""slots": [{ "name": "Upper" }, { "name": "Upper", "mask": [""] }],
           "montages": [
             { "name": "A", "clip": "" , "slot": "Legs", "blend_out": -1 },
             { "name": "Throw", "clip": "T", "events": [{ "time": -1, "name": "x" }],
               "sections": [{ "name": "Wind", "time": 0.5, "next": "Hodl" },
                            { "name": "Wind", "time": 0.2 }] },
             { "name": "A", "clip": "T" }]"#,
    );
    let found = problems(&g);
    for expected in [
        "slots[1].name: duplicate slot 'Upper'",
        "slots[1].mask: an empty bone name",
        "montages[0].clip: empty",
        "montages[0].slot: no slot 'Legs'",
        "montages[0].blend_out: negative or non-finite",
        "montages[1].sections[0].next: no section 'Hodl'",
        "montages[1].sections[1].name: duplicate section 'Wind'",
        "montages[1].sections[1].time: not after the section before it",
        "montages[2].name: duplicate montage 'A'",
    ] {
        assert!(
            found.iter().any(|p| p == expected),
            "{expected} not in {found:#?}"
        );
    }
    assert!(
        found.iter().any(|p| p.starts_with("montages[1].events:")),
        "{found:#?}"
    );
}
