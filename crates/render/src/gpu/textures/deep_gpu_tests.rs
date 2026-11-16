//! A normal map kept at 16 bits uploads at 16 (#2257): the issue's repro, a faint
//! fbm ripple through `bump_to_normal {strength = 0.0005}`, baked as `normal16`,
//! reads back from the GPU with many times the distinct slopes its 8-bit bake
//! keeps, and makes no BC5 cache that would round it back to 8 bits.

use std::collections::BTreeSet;
use std::path::Path;

use super::deep::DEEP_FORMAT;
use super::freshness_tests::temp_png;
use super::TextureKind;
use crate::readback::{read_texture_bytes, read_texture_rgba8};
use rusty_asset::sidecar::{set_texture, Compression, TextureSettings, TextureType};
use rusty_asset::texture_import::refresh;
use rusty_asset::MapRef;
use rusty_core::color::f16_to_f32;
use rusty_procgen::recipe::{BumpToNormal, Node, Noise, NoiseKind, OpKind};
use rusty_procgen::{bake::bake_to_png, evaluate, Slot, TextureRecipe};

const SIDE: u32 = 128;

/// The issue's repro at `SIDE`², baked for `slot` to a `NormalMap` PNG at a fresh
/// path, with `compression`.
fn ripple(name: &str, slot: Slot, compression: Compression) -> String {
    let noise = Noise {
        kind: NoiseKind::Fbm,
        scale: 60.0,
        aspect: [1.0, 1.0],
        octaves: 2,
        lacunarity: 2.0,
        gain: 0.5.into(),
        distortion: 0.0,
        seed: 0,
    };
    let node = |id: &str, op, inputs: &[&str]| Node {
        id: id.into(),
        op,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
    };
    let recipe = TextureRecipe::new(
        SIDE,
        vec![
            node("h", OpKind::Noise(noise), &[]),
            node(
                "n",
                OpKind::BumpToNormal(BumpToNormal {
                    strength: 0.0005.into(),
                }),
                &["h"],
            ),
        ],
    );
    let path = temp_png(name);
    bake_to_png(&evaluate(&recipe).unwrap(), slot, &path).unwrap();
    let settings = TextureSettings {
        texture_type: TextureType::NormalMap,
        compression,
        ..Default::default()
    };
    set_texture(Path::new(&path), settings).unwrap();
    path
}

/// How many distinct tangent-space (x, y) the base level holds: the facets.
fn facets(texels: impl Iterator<Item = [u32; 2]>) -> usize {
    texels.collect::<BTreeSet<_>>().len()
}

#[test]
fn gpu_a_16_bit_normal_map_keeps_the_slopes_8_bits_terrace() {
    let Some(renderer) = crate::test_gpu::headless_or_skip(16, 16) else {
        return;
    };
    let (device, queue) = (&renderer.device, &renderer.queue);
    let deep = ripple("deep_ripple_2257", Slot::Normal16, Compression::Normal);
    assert_eq!(refresh(Path::new(&deep), ""), Ok(false), "no BC5 cache");
    let deep = renderer.upload_map(&MapRef::from(deep.as_str()), TextureKind::Normal);
    let deep = deep.unwrap();
    assert_eq!(deep.texture.format(), DEEP_FORMAT);
    let bytes = read_texture_bytes(device, queue, &deep.texture, (SIDE, SIDE), 8);
    let half = |b: &[u8]| f16_to_f32(u16::from_le_bytes([b[0], b[1]])).to_bits();
    let deep_facets = facets(bytes.chunks(8).map(|t| [half(&t[0..2]), half(&t[2..4])]));

    let flat = ripple("flat_ripple_2257", Slot::Normal, Compression::None);
    let flat = renderer.upload_map(&MapRef::from(flat.as_str()), TextureKind::Normal);
    let flat = flat.unwrap();
    assert_ne!(
        flat.texture.format(),
        DEEP_FORMAT,
        "an 8-bit map stays 8-bit"
    );
    let bytes = read_texture_rgba8(device, queue, &flat.texture, SIDE, SIDE);
    let flat_facets = facets(bytes.chunks(4).map(|t| [t[0].into(), t[1].into()]));

    eprintln!("distinct (x, y): 8-bit {flat_facets}, 16-bit {deep_facets}");
    assert!(
        flat_facets <= 16,
        "the repro terraces at 8 bits: {flat_facets}"
    );
    assert!(
        deep_facets >= 16 * flat_facets,
        "16 bits keep the slopes: {deep_facets} against {flat_facets}"
    );
}
