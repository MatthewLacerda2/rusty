//! `AssetRef` (#1295): a reference splits on its first `::`, and the path in front
//! of it must end in a model extension; a sub id may hold `::`, a path may not, and
//! the builder and the parser refuse the same things with a reason.

use super::{AssetRef, ImportError};

fn why(result: Result<AssetRef, ImportError>) -> String {
    match result {
        Err(ImportError::BadReference(why)) => why,
        other => panic!("expected a bad reference, got {other:?}"),
    }
}

#[test]
fn sub_id_keeps_its_own_separators() {
    let r = AssetRef::new("assets/hero.glb", "Body::Mesh").unwrap();
    assert_eq!(r.to_string_ref(), "assets/hero.glb::Body::Mesh");
    assert_eq!(AssetRef::parse(&r.to_string_ref()).unwrap(), r);
    let parsed = AssetRef::parse("models/Crate.FBX::a::b::c").unwrap();
    assert_eq!(
        (parsed.path.as_str(), parsed.sub_object.as_str()),
        ("models/Crate.FBX", "a::b::c")
    );
}

#[test]
fn path_holding_a_separator_is_refused_with_a_reason() {
    let parsed = why(AssetRef::parse("dir::x/crates.glb::Barrel"));
    assert!(parsed.contains("may not contain `::`"), "{parsed}");
    let built = why(AssetRef::new("my::kit/crate.glb", "Barrel"));
    assert!(
        built.starts_with("'my::kit/crate.glb::Barrel': "),
        "{built}"
    );
    assert!(built.contains("may not contain `::`"), "{built}");
}

#[test]
fn malformed_references_are_refused_in_one_shape() {
    let not_model = why(AssetRef::parse("notes.txt::Barrel"));
    assert!(not_model.contains("model extension"), "{not_model}");
    assert!(why(AssetRef::new("notes.txt", "Barrel")).contains("model extension"));
    for reference in ["no_separator_here", "::onlysub", "a.glb::"] {
        let w = why(AssetRef::parse(reference));
        assert!(w.starts_with(&format!("'{reference}': expected")), "{w}");
    }
    assert!(why(AssetRef::new("a.glb", "")).contains("expected `path::sub_object`"));
}
