//! The compressor as a document: what a recipe may leave out.

use rusty_zimmer::patch::Fx;

#[test]
fn a_compressor_carries_its_documented_defaults() {
    let json = r#"{ "fx": "compress", "threshold": -18.0, "ratio": 4.0 }"#;
    match serde_json::from_str::<Fx>(json).expect("parses") {
        Fx::Compress {
            attack,
            release,
            makeup,
            mix,
            sidechain,
            ..
        } => {
            assert_eq!(attack, 0.01, "10 ms");
            assert_eq!(release, 0.15, "150 ms");
            assert_eq!(makeup, 0.0, "nothing is handed back unasked");
            assert_eq!(mix, 1.0, "and none of the signal goes past it");
            assert!(sidechain.is_none(), "so it listens to itself");
        }
        other => panic!("wrong effect: {other:?}"),
    }
}
