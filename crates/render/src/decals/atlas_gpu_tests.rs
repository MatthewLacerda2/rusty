use super::{DecalAtlas, LAYER_SIZE, MIPS};
use crate::gpu::textures::TextureKind;
use crate::readback::read_mip_rgba8;
use crate::Renderer;
use rusty_asset::MapRef;
use rusty_core::fs::freshness::Freshness;

fn checker(name: &str) -> MapRef {
    let dir = rusty_test_support::temp::dir().join("rusty_1364");
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

fn level(renderer: &Renderer, atlas: &DecalAtlas, layer: u32, mip: u32) -> Vec<u8> {
    let side = (LAYER_SIZE >> mip).max(1);
    let (device, queue) = (&renderer.device, &renderer.queue);
    read_mip_rgba8(device, queue, (&atlas.texture, mip, layer), (side, side))
}

fn all_near(texels: &[u8], want: u8) -> bool {
    texels
        .chunks(4)
        .all(|t| t[..3].iter().all(|&c| c.abs_diff(want) <= 1))
}

#[test]
fn gpu_an_albedo_layer_averages_in_linear_light_and_a_data_layer_its_bytes() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(4, 4) else {
        return;
    };
    let (data, colour) = (checker("data"), checker("colour"));
    let mut atlas = DecalAtlas::new(&renderer.device);
    let gpu = (
        &renderer.device,
        (&renderer.queue, &renderer.staging),
        &renderer.mip_generator,
    );
    let maps = [(&data, TextureKind::Data), (&colour, TextureKind::Colour)];
    atlas.prepare(gpu, &maps, &mut Freshness::new());
    assert_eq!(
        (atlas.layer(data.as_str()), atlas.layer(colour.as_str())),
        (0, 1)
    );
    for mip in [1, MIPS - 1] {
        let (c, d) = (
            level(&renderer, &atlas, 1, mip),
            level(&renderer, &atlas, 0, mip),
        );
        assert!(all_near(&c, 188), "albedo mip {mip}: {:?}", &c[..8]);
        assert!(all_near(&d, 128), "data mip {mip}: {:?}", &d[..8]);
    }
    let base = level(&renderer, &atlas, 1, 0);
    assert_eq!((base[0], base[4]), (0, 255), "base level {:?}", &base[..8]);
}
