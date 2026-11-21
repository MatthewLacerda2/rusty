//! The compressor as a document: what a recipe may leave out.

use scorsese_zimmer::patch::Fx;

/// A compressor written the short way — the two numbers that say what it does,
/// and four that fill themselves in. Those four *are* audio, so they are
/// asserted here rather than left to the derive: a default that moved would
/// change what every recipe using the short form bakes to, and a bake is
/// addressed by the hash of its recipe.
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
