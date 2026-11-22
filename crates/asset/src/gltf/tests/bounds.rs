//! An accessor whose elements run past what holds them: refused before the `gltf`
//! crate's readers compute its end unchecked.

use super::malformed::animated_glb;
use crate::gltf::import::import_slice;

fn refused(bytes: &[u8]) -> String {
    import_slice(bytes).unwrap_err().to_string()
}

#[test]
fn an_accessor_or_view_running_past_what_holds_it_is_refused() {
    let huge = animated_glb(|d| d["accessors"][0]["count"] = 2_000_000_000_000_000_000u64.into());
    let err = refused(&huge);
    assert!(
        err.contains("accessor 0") && err.contains("runs past"),
        "{err}"
    );
    let view = animated_glb(|d| d["bufferViews"][5]["byteOffset"] = 250.into());
    assert!(refused(&view).contains("accessor 5"));
}

/// The translation values (accessor 5) with one sparse value over the first key.
fn sparse_glb(count: u64, edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    animated_glb(|d| {
        d["accessors"][5]["sparse"] = serde_json::json!({ "count": count,
            "indices": { "bufferView": 4, "componentType": 5121 },
            "values": { "bufferView": 5 } });
        edit(d);
    })
}

#[test]
fn a_sparse_accessor_of_no_elements_or_on_a_strided_view_is_refused() {
    let valid = import_slice(&sparse_glb(1, |_| {})).unwrap();
    assert_eq!(
        valid.validate(),
        Ok(()),
        "a well-formed sparse accessor imports"
    );
    assert!(refused(&sparse_glb(0, |_| {})).contains("sparse count of 0"));
    let strided = sparse_glb(1, |d| d["bufferViews"][5]["byteStride"] = 12.into());
    assert!(refused(&strided).contains("byteStride"));
}
