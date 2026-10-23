//! A conversion notes what it swapped in, in the write log (#1219): the cached
//! `.glb`, each file of its textures folder, and the source, so a path-keyed
//! cache drops what it read from the old conversion.

use super::super::{refresh, stage, Converted, Converter, Staging};
use crate::asset::ImportError;
use crate::core::fs::written::written_since;
use std::path::Path;

/// A converter that copies its source to the `.glb` and to one texture.
const WITH_TEXTURE: Converter = Converter {
    extension: "src",
    name: "copy-with-texture",
    version: 1,
    convert: with_texture,
};

fn with_texture(source: &Path, to: &Staging) -> Result<Converted, ImportError> {
    let err = |e: std::io::Error| ImportError::Parse(e.to_string());
    let bytes = std::fs::read(source).map_err(err)?;
    stage::write(&to.glb, &bytes).map_err(err)?;
    std::fs::create_dir(&to.textures).map_err(err)?;
    stage::write(&to.textures.join("skin.png"), &bytes).map_err(err)?;
    Ok(Converted::default())
}

/// The paths noted while `refresh` re-made `out` from `source`.
fn noted_by_refresh(source: &Path, out: &Path) -> Vec<String> {
    let (start, _) = written_since(u64::MAX);
    refresh(&WITH_TEXTURE, source, out).unwrap();
    written_since(start).1
}

#[test]
fn a_conversion_notes_the_glb_its_textures_and_the_source() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("a.src"), dir.join("cache/a.src.glb"));
    std::fs::write(&source, "one").unwrap();
    for _ in 0..2 {
        let noted = noted_by_refresh(&source, &out);
        let texture = super::super::textures_dirs(&out)
            .into_iter()
            .map(|d| d.join("skin.png").to_string_lossy().into_owned())
            .find(|t| Path::new(t).is_file())
            .expect("a texture swapped in");
        for path in [out.to_string_lossy(), source.to_string_lossy()] {
            assert!(noted.contains(&path.into_owned()), "{noted:?}");
        }
        assert!(noted.contains(&texture), "{texture} in {noted:?}");
        assert!(!noted.iter().any(|p| p.contains("staging")), "{noted:?}");
        std::fs::write(&source, "two").unwrap();
    }
}

#[test]
fn a_current_cache_notes_nothing() {
    let dir = crate::test_temp::dir();
    let (source, out) = (dir.join("b.src"), dir.join("cache/b.src.glb"));
    std::fs::write(&source, "one").unwrap();
    refresh(&WITH_TEXTURE, &source, &out).unwrap();
    let noted = noted_by_refresh(&source, &out);
    assert!(
        !noted.contains(&out.to_string_lossy().into_owned()),
        "{noted:?}"
    );
}
