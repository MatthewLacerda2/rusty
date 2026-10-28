//! Unit tests for the texture recipe document (`recipe/mod.rs`).

use super::*;

fn sample_recipe() -> TextureRecipe {
    TextureRecipe::new(
        64,
        vec![
            Node {
                id: "n0".into(),
                op: OpKind::Checker(Checker {
                    tiles: 4,
                    color_a: [0.0, 0.0, 0.0, 1.0],
                    color_b: [1.0, 1.0, 1.0, 1.0],
                }),
                inputs: vec![],
            },
            Node {
                id: "n1".into(),
                op: OpKind::Invert,
                inputs: vec!["n0".into()],
            },
        ],
    )
    .with_seed(7)
    .with_output("n1")
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
        OpKind::Voronoi(Voronoi { output, .. }) => assert_eq!(output, VoronoiOutput::Distance),
        other => panic!("wrong op: {other:?}"),
    }
}

#[test]
fn outputs_stay_out_of_single_output_json() {
    let single = sample_recipe().to_json().unwrap();
    assert!(!single.contains("outputs"), "{single}");
}
