//! A decal layer's mips average as what it holds (#1364): an albedo in linear
//! light, a data map its bytes, as material maps do (#1322). Before, every layer's
//! chain was the CPU averaging the sRGB bytes, so a high-contrast albedo decal
//! darkened with distance. Skips with no adapter.

use super::{DecalAtlas, LAYER_SIZE, MIPS};
use crate::asset::MapRef;
use crate::core::fs::freshness::Freshness;
use crate::render::gpu::textures::TextureKind;
use crate::render::readback::read_layer_mip_rgba8;
use crate::render::Renderer;

/// A layer-sized black and white checker of 1-texel cells saved under `name`, so
/// the layer's base is the image itself and every mip below it an even grey.
fn checker(name: &str) -> MapRef {
    let dir = crate::test_temp::dir().join("rusty_1364");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.png"));
    image::RgbaImage::from_fn(LAYER_SIZE, LAYER_SIZE, |x, y| match (x + y) % 2 {
        0 => image::Rgba([0, 0, 0, 255]),
        _ => image::Rgba([255, 255, 255, 255]),
    })
    .save(&path)
    .unwrap();
    MapRef::new(path.to_string_lossy().into_owned())
}

/// Every texel of `layer`'s level `mip`, RGBA8.
fn level(renderer: &Renderer, atlas: &DecalAtlas, layer: u32, mip: u32) -> Vec<u8> {
    let side = (LAYER_SIZE >> mip).max(1);
    let (device, queue) = (&renderer.device, &renderer.queue);
    read_layer_mip_rgba8(device, queue, (&atlas.texture, mip, layer), (side, side))
}

/// Whether every RGB channel of `texels` is within 1 of `want`.
fn all_near(texels: &[u8], want: u8) -> bool {
    texels
        .chunks(4)
        .all(|t| t[..3].iter().all(|&c| c.abs_diff(want) <= 1))
}

#[test]
fn gpu_an_albedo_layer_averages_in_linear_light_and_a_data_layer_its_bytes() {
    let Some(renderer) = crate::render::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let (data, colour) = (checker("data"), checker("colour"));
    let mut atlas = DecalAtlas::new(&renderer.device);
    let gpu = (&renderer.device, &renderer.queue, &renderer.mip_generator);
    // The data map first, so the albedo lands on layer 1: the pass draws one layer
    // of the array, not its first.
    let maps = [(&data, TextureKind::Data), (&colour, TextureKind::Colour)];
    atlas.prepare(gpu, &maps, &mut Freshness::new());
    assert_eq!(
        (atlas.layer(data.as_str()), atlas.layer(colour.as_str())),
        (0, 1)
    );
    // Half the light of white is sRGB 188; the byte midpoint is 128.
    for mip in [1, MIPS - 1] {
        let (c, d) = (
            level(&renderer, &atlas, 1, mip),
            level(&renderer, &atlas, 0, mip),
        );
        assert!(all_near(&c, 188), "albedo mip {mip}: {:?}", &c[..8]);
        assert!(all_near(&d, 128), "data mip {mip}: {:?}", &d[..8]);
    }
    // The base is the image as written.
    let base = level(&renderer, &atlas, 1, 0);
    assert_eq!((base[0], base[4]), (0, 255), "base level {:?}", &base[..8]);
}
