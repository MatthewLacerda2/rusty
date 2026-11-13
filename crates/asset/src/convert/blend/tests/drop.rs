//! Leaving an unreadable made image out of the glTF (#1072): the image, the
//! textures that sample it and the material slots that sample those go, and every
//! index after them is renumbered.

use super::super::drop;
use serde_json::json;

#[test]
fn a_dropped_image_takes_its_textures_and_slots_and_the_rest_renumber() {
    let mut doc = json!({
        "images": [{"uri": "a.png"}, {"uri": "bad"}, {"uri": "c.png"}],
        "textures": [{"source": 1}, {"source": 2}, {"source": 0}],
        "materials": [{
            "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}},
            "normalTexture": {"index": 1, "scale": 1.0},
            "emissiveTexture": {"index": 2},
        }],
    });
    drop::images(&mut doc, &[1]);
    let want = json!({
        "images": [{"uri": "a.png"}, {"uri": "c.png"}],
        "textures": [{"source": 1}, {"source": 0}],
        "materials": [{
            "pbrMetallicRoughness": {},
            "normalTexture": {"index": 0, "scale": 1.0},
            "emissiveTexture": {"index": 1},
        }],
    });
    assert_eq!(doc, want);
}

#[test]
fn nothing_dropped_leaves_the_gltf_as_it_was() {
    let mut doc = json!({"images": [{"uri": "a.png"}], "textures": [{"source": 0}]});
    let before = doc.clone();
    drop::images(&mut doc, &[]);
    assert_eq!(doc, before);
}
