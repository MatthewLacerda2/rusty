use std::path::Path;

use super::compressed::FEATURE;
use super::TextureKind;
use rusty_asset::convert::{cached_gltf, texture_ref};
use rusty_asset::texture_import::refresh_all;
use rusty_asset::MapRef;
use rusty_test_support::models::{embedded_fbx, png, Embedded};
use wgpu::TextureFormat as F;

#[test]
fn gpu_an_extracted_texture_uploads_compressed_once_imported() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let dir = rusty_test_support::temp::dir();
    let (albedo, bumps) = (png(16, 3), png(16, 9));
    let fbx = dir.join("m.fbx");
    let maps = [
        Embedded {
            name: "Albedo",
            property: "DiffuseColor",
            png: &albedo,
        },
        Embedded {
            name: "Bumps",
            property: "NormalMap",
            png: &bumps,
        },
    ];
    std::fs::write(&fbx, embedded_fbx(&maps)).unwrap();
    cached_gltf(&fbx).expect("the model converts");
    let source = fbx.to_string_lossy();
    let map = |identity: &str| MapRef::from(texture_ref::reference(&source, identity));
    let (colour, normal) = (map("Albedo"), map("Bumps"));
    let format = |r: &mut crate::Renderer, m: &MapRef, kind| {
        r.refresh_textures();
        r.load_texture_as(m, kind).texture.format()
    };
    assert!(!format(&mut renderer, &colour, TextureKind::Colour).is_compressed());
    format(&mut renderer, &normal, TextureKind::Normal);
    let bc = renderer.device.features().contains(FEATURE);
    let root = Path::new(&dir);
    assert_eq!(
        renderer.uncached_textures(root).count,
        if bc { 2 } else { 0 }
    );
    assert_eq!(refresh_all(root).encoded, 2);
    let colour = format(&mut renderer, &colour, TextureKind::Colour);
    let normal = format(&mut renderer, &normal, TextureKind::Normal);
    if !bc {
        assert!(!colour.is_compressed(), "no BC here");
        return;
    }
    assert_eq!(colour, F::Bc7RgbaUnormSrgb, "the import's note swapped it");
    assert_eq!(
        normal,
        F::Bc5RgUnorm,
        "a normal map is not encoded as colour"
    );
    assert_eq!(renderer.uncached_textures(root).count, 0);
}
