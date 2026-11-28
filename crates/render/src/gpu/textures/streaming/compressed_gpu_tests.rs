use super::gpu_tests::{boxes, dropped, gradient, settle, RES};
use crate::gpu::textures::compressed::FEATURE;
use crate::scene::Variant;
use rusty_asset::sidecar::{set_texture, TextureSettings, TextureType};
use rusty_asset::texture_import::{refresh, shift_variance};
use rusty_asset::MapRef;

fn imported(name: &str, texture_type: TextureType) -> MapRef {
    let map = gradient(name, 64);
    let file = map.file().unwrap();
    let settings = TextureSettings {
        texture_type,
        ..TextureSettings::default()
    };
    set_texture(&file, settings).unwrap();
    refresh(&file, "").unwrap();
    map
}

#[test]
fn gpu_a_compressed_texture_streams_down_and_back_whole_blocks() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    if !r.device.features().contains(FEATURE) {
        return;
    }
    r.stream_textures(true);
    let map = imported("stream_bc7", TextureType::Default);
    settle(&mut r, &boxes(&[(&map, 150.0)]), RES, &[&map]);
    let far = &r.gpu_textures[&map].texture;
    assert!(far.format().is_compressed());
    assert_eq!(
        (far.size().width, dropped(&r, &map)),
        (4, 4),
        "down to one 4×4 block"
    );
    settle(&mut r, &boxes(&[(&map, 2.0)]), RES, &[&map]);
    assert_eq!(dropped(&r, &map), 0, "read back from the cache whole");
    assert_eq!(r.gpu_textures[&map].texture.mip_level_count(), 7);
}

#[test]
fn gpu_a_streamed_normal_map_moves_its_variance_and_its_records_follow() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(RES, RES) else {
        return;
    };
    if !r.device.features().contains(FEATURE) {
        return;
    }
    let map = imported("stream_bc5", TextureType::NormalMap);
    let mut scene = boxes(&[(&gradient("stream_albedo", 4), 150.0)]);
    scene.materials.get_mut("m0").unwrap().normal_map = Some(map.clone());
    let record = |r: &crate::Renderer| {
        let slots = r.retained.records.variants_of(scene.id(), "m0");
        let (_, slot) = slots.into_iter().find(|(v, _)| *v == Variant::Lit).unwrap();
        r.retained.records.table().get(slot).normal_variance
    };
    super::gpu_tests::draw(&mut r, &scene, RES);
    let whole = r.gpu_textures[&map]
        .normal_variance
        .expect("a BC5 upload's table");
    let before = record(&r);
    r.stream_textures(true);
    settle(&mut r, &scene, RES, &[&map]);
    let by = dropped(&r, &map);
    assert!(by > 0);
    let table = r.gpu_textures[&map].normal_variance.unwrap();
    assert_eq!(
        table,
        shift_variance(whole, by),
        "the table starts at the new top"
    );
    assert_ne!(record(&r), before, "the record reads the moved table");
}
