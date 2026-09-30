//! Unit tests for the texture recipe document (`recipe/mod.rs`).

use super::*;

fn sample_recipe() -> TextureRecipe {
    TextureRecipe {
        resolution: 64,
        seed: 7,
        nodes: vec![
            Node {
                id: "n0".into(),
                op: OpKind::Checker {
                    tiles: 4,
                    color_a: [0.0, 0.0, 0.0, 1.0],
                    color_b: [1.0, 1.0, 1.0, 1.0],
                },
                inputs: vec![],
            },
            Node {
                id: "n1".into(),
                op: OpKind::Invert,
                inputs: vec!["n0".into()],
            },
        ],
        output: Some("n1".into()),
    }
}

#[test]
fn recipe_round_trips_through_json_unchanged() {
    let r = sample_recipe();
    let json = r.to_json().expect("serialize");
    let back = TextureRecipe::from_json(&json).expect("deserialize");
    assert_eq!(r, back);
}

#[test]
fn op_tag_is_snake_case() {
    let json = serde_json::to_string(&OpKind::WhiteNoise).unwrap();
    assert!(json.contains("\"white_noise\""), "got {json}");
}

#[test]
fn voronoi_output_defaults_to_distance_when_absent() {
    let json = r#"{ "id": "v", "op": "voronoi", "scale": 8.0 }"#;
    let node: Node = serde_json::from_str(json).expect("parse");
    match node.op {
        OpKind::Voronoi { output, .. } => assert_eq!(output, VoronoiOutput::Distance),
        other => panic!("wrong op: {other:?}"),
    }
}

#[test]
fn an_unknown_recipe_key_is_refused() {
    let err =
        TextureRecipe::from_json(r#"{ "resolution": 8, "nodes": [], "seeed": 3 }"#).unwrap_err();
    assert!(err.contains("seeed"), "{err}");
}
