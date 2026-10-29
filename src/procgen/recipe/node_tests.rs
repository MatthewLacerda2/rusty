//! Strict node parsing (#395) and the node-named errors (#1575).

use super::*;

fn parse(json: &str) -> Result<Node, String> {
    serde_json::from_str(json).map_err(|e| e.to_string())
}

#[test]
fn a_misspelled_op_param_is_refused_by_name() {
    let err = parse(r#"{ "id": "n", "op": "noise", "kind": "fbm", "scale": 8, "scael": 8 }"#)
        .unwrap_err();
    assert!(err.contains("\"scael\""), "{err}");
    assert!(err.contains("scale") && err.contains("octaves"), "{err}");
}

#[test]
fn a_param_on_a_paramless_op_is_refused() {
    let err = parse(r#"{ "id": "n", "op": "invert", "amount": 1 }"#).unwrap_err();
    assert!(
        err.contains("\"amount\"") && err.contains("no params"),
        "{err}"
    );
}

#[test]
fn known_keys_and_omitted_defaults_still_parse() {
    let node = parse(r#"{ "id": "v", "op": "voronoi", "scale": 4, "inputs": [] }"#);
    assert!(node.is_ok(), "{node:?}");
}

#[test]
fn a_wrong_input_count_is_refused_at_parse_time() {
    let short = parse(r#"{ "id": "m", "op": "mix", "mode": "add", "inputs": ["a"] }"#);
    let err = short.unwrap_err();
    assert!(err.starts_with("node \"m\" (mix): takes 2 to 3"), "{err}");
    let surplus = parse(r#"{ "id": "i", "op": "invert", "inputs": ["a", "b"] }"#);
    assert!(surplus.unwrap_err().contains("2 given"));
}

#[test]
fn a_unary_math_func_refuses_a_second_input() {
    let math = |func: &str, inputs: &str| {
        parse(&format!(
            r#"{{ "id": "m", "op": "math", "func": "{func}", "inputs": [{inputs}] }}"#
        ))
    };
    for func in ["abs", "fract", "sqrt"] {
        let err = math(func, r#""a", "b""#).unwrap_err();
        assert!(
            err.contains("\"m\"") && err.contains(&format!("func \"{func}\"")),
            "{err}"
        );
        assert!(err.contains("takes 1 input, but 2 given"), "{err}");
        assert!(math(func, r#""a""#).is_ok(), "{func}");
    }
    assert!(math("max", r#""a", "b""#).is_ok());
}

#[test]
fn a_link_on_a_param_that_takes_none_names_the_ones_that_do() {
    let link = r#"{ "node": "m" }"#;
    let noise = format!(r#"{{ "id": "n", "op": "noise", "kind": "fbm", "scale": {link} }}"#);
    let err = parse(&noise).unwrap_err();
    assert!(
        err.contains("\"scale\" takes no link; noise links gain"),
        "{err}"
    );
    let mix = format!(r#"{{ "id": "x", "op": "mix", "mode": "add", "factor": {link} }}"#);
    assert!(parse(&mix).unwrap_err().contains("mix takes no links"));
    let warp =
        format!(r#"{{ "id": "w", "op": "warp", "strength": {link}, "inputs": ["a", "b"] }}"#);
    assert!(parse(&warp).is_ok());
}

#[test]
fn a_misspelled_ramp_stop_key_is_refused() {
    let json = r#"{ "id": "r", "op": "color_ramp", "stops": [{ "pos": 0, "colour": [0,0,0,1] }] }"#;
    assert!(parse(json).unwrap_err().contains("colour"));
}

#[test]
fn every_parse_error_names_the_node_and_its_op() {
    let missing = parse(r#"{ "id": "s", "op": "separate_rgb", "inputs": ["a"] }"#);
    let err = missing.unwrap_err();
    assert!(
        err.starts_with(r#"node "s" (separate_rgb): missing field `channel`"#),
        "{err}"
    );
    let typo = parse(r#"{ "id": "n", "op": "noise", "kind": "fbm", "scale": 8, "scael": 8 }"#);
    assert!(typo
        .unwrap_err()
        .starts_with(r#"node "n" (noise): unknown key "scael""#));
    let link = parse(r#"{ "id": "n", "op": "noise", "kind": "fbm", "scale": { "node": "m" } }"#);
    assert!(link
        .unwrap_err()
        .starts_with(r#"node "n" (noise): param "scale""#));
    let op = parse(r#"{ "id": "q", "op": "mapp" }"#).unwrap_err();
    assert!(
        op.starts_with(r#"node "q" (mapp): unknown variant"#),
        "{op}"
    );
}

#[test]
fn a_mapping_leaves_out_any_transform_as_the_identity() {
    let node = parse(r#"{ "id": "m", "op": "mapping", "rotation": 0.25, "inputs": ["c"] }"#);
    let OpKind::Mapping(m) = node.unwrap().op else {
        panic!()
    };
    assert_eq!(
        (m.scale, m.rotation, m.translation),
        ([1.0; 2], 0.25, [0.0; 2])
    );
    assert!(m.tiling);
}
