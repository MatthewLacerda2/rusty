//! A texture left out on purpose is reported in `Converted::skipped` by the
//! converter itself (#1197), not only by `refresh` replaying it.

use super::fixture;
use crate::asset::convert::fbx::convert;
use crate::asset::convert::{Skip, Staging};

/// What converting the fixture `name` into a fresh folder skipped.
fn skipped(name: &str) -> Vec<Skip> {
    let dir = crate::test_temp::dir();
    let to = Staging {
        glb: dir.join("out.glb"),
        textures: dir.join("out.glb.textures"),
        out: dir.join("out.glb"),
    };
    convert(&fixture(name), &to)
        .expect("the fixture converts")
        .skipped
}

#[test]
fn a_texture_neither_on_disk_nor_embedded_is_skipped_by_name() {
    let skip = Skip {
        what: "FBX texture `Gone`".into(),
        why: "neither a file on disk nor embedded".into(),
    };
    assert_eq!(skipped("missing_texture.fbx"), [skip]);
}

#[test]
fn an_embedded_format_the_importer_cannot_read_is_skipped_by_name() {
    let [skip] = skipped("embedded_textures.fbx")
        .try_into()
        .expect("one skip");
    assert_eq!(skip.what, "FBX texture `Bumps`");
    assert!(skip.why.starts_with("embeds a `.dds`"), "{}", skip.why);
}
