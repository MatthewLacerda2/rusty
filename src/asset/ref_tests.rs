//! `AssetRef::parse` splits on the first `::` after the file's model extension
//! (#1295), so a sub id may hold `::` and a path may not.

use super::{AssetRef, ImportError};

fn parsed(reference: &str) -> (String, String) {
    let r = AssetRef::parse(reference).unwrap();
    (r.path, r.sub_object)
}

#[test]
fn sub_id_keeps_its_own_separators() {
    let r = AssetRef {
        path: "assets/hero.glb".into(),
        sub_object: "Body::Mesh".into(),
    };
    assert_eq!(r.to_string_ref(), "assets/hero.glb::Body::Mesh");
    assert_eq!(AssetRef::parse(&r.to_string_ref()).unwrap(), r);
    assert_eq!(
        parsed("models/Crate.FBX::a::b::c"),
        ("models/Crate.FBX".into(), "a::b::c".into())
    );
}

#[test]
fn path_holding_a_separator_is_refused_with_a_reason() {
    let Err(ImportError::BadReference(why)) = AssetRef::parse("dir::x/crates.glb::Barrel") else {
        panic!("a path with `::` must not parse");
    };
    assert!(why.contains("may not contain `::`"), "{why}");
}

#[test]
fn path_without_a_model_extension_is_refused() {
    let Err(ImportError::BadReference(why)) = AssetRef::parse("notes.txt::Barrel") else {
        panic!("a non-model path must not parse");
    };
    assert!(why.contains("model extension"), "{why}");
}
