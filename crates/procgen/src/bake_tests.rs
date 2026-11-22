use super::*;

#[test]
fn slot_parse_is_case_and_separator_insensitive() {
    assert_eq!(Slot::parse("BaseColor"), Ok(Slot::BaseColor));
    assert_eq!(Slot::parse("albedo"), Ok(Slot::BaseColor));
    assert_eq!(
        Slot::parse("metallic-roughness"),
        Ok(Slot::MetallicRoughness)
    );
    assert_eq!(Slot::parse("orm"), Ok(Slot::MetallicRoughness));
    assert_eq!(Slot::parse("data"), Ok(Slot::Data));
}

#[test]
fn every_slot_is_in_all_and_parses_back_from_its_name() {
    // Exhaustive: a new variant fails to build here until it joins `ALL`.
    let listed = |s: Slot| match s {
        Slot::BaseColor | Slot::Emissive | Slot::Normal | Slot::Normal16 => Slot::ALL.contains(&s),
        Slot::Roughness | Slot::Metallic | Slot::MetallicRoughness => Slot::ALL.contains(&s),
        Slot::Occlusion | Slot::Specular | Slot::Height | Slot::Data => Slot::ALL.contains(&s),
    };
    for s in Slot::ALL {
        assert!(listed(s));
        assert_eq!(Slot::parse(s.name()), Ok(s), "{s:?}");
    }
}

#[test]
fn an_unknown_slot_is_refused_by_name() {
    let err = Slot::parse("basecolour").unwrap_err();
    assert!(err.contains("\"basecolour\""), "{err}");
    assert!(err.contains("base_color") && err.contains("data"), "{err}");
}

#[test]
fn srgb_encoding_applied_only_to_color_slots() {
    // Mid linear gray (0.5) encodes brighter under sRGB (~0.735 → 188).
    let img = Image::filled(2, [0.5, 0.5, 0.5, 1.0]);
    let color = encode(&img, Slot::BaseColor).get_pixel(0, 0).0;
    let data = encode(&img, Slot::Data).get_pixel(0, 0).0;
    assert_eq!(data[0], 128, "linear data: 0.5 -> 128");
    assert!(
        color[0] > 180 && color[0] < 195,
        "sRGB 0.5 ~ 188, got {}",
        color[0]
    );
}

#[test]
fn a_specular_level_reaches_alpha_and_occlusion_passes_through() {
    let img = Image::filled(2, [0.25, 0.5, 0.75, 1.0]);
    let level = encode(&img, Slot::Specular).get_pixel(0, 0).0;
    assert_eq!(level, [to_u8(0.25); 4], "red to every channel");
    let ao = encode(&img, Slot::Occlusion).get_pixel(0, 0).0;
    assert_eq!(ao, [to_u8(0.25), to_u8(0.5), to_u8(0.75), 255]);
    assert_eq!(Slot::parse("ao"), Ok(Slot::Occlusion));
    assert_eq!(Slot::Occlusion.material_maps(), ["occlusion_map"]);
    assert_eq!(
        Slot::parse("specular").unwrap().material_maps(),
        ["specular_map"]
    );
}

#[test]
fn metallic_roughness_packs_metallic_to_b_roughness_to_g() {
    let img = Image::filled(2, [1.0, 0.25, 0.0, 1.0]);
    let px = encode(&img, Slot::MetallicRoughness).get_pixel(0, 0).0;
    assert_eq!(px[2], 255, "metallic -> B");
    assert_eq!(px[1], to_u8(0.25), "roughness -> G");
    assert_eq!(px[0], 0, "R cleared in the pack");
}

#[test]
fn a_data_ramp_round_trips_at_16_bit_precision() {
    // A 0..1 ramp across 256² pixels: 65536 distinct values, past what 8 bits hold.
    let res = 256;
    let n = (res * res) as f32;
    let mut img = Image::new(res);
    for (i, p) in img.pixels_mut().iter_mut().enumerate() {
        *p = [i as f32 / (n - 1.0); 4];
    }
    let path = rusty_test_support::temp::dir().join("rusty_bake_data_ramp_1491.png");
    bake_to_png(&img, Slot::Data, path.to_str().unwrap()).unwrap();
    let back = image::open(&path).unwrap();
    assert_eq!(back.color(), image::ColorType::Rgba16, "data bakes 16-bit");
    let back = back.to_rgba16();
    for (p, q) in img.pixels().iter().zip(back.pixels()) {
        let read = f32::from(q.0[0]) / 65535.0;
        assert!(
            (read - p[0]).abs() <= 0.5 / 65535.0 + 1e-7,
            "{} vs {read}",
            p[0]
        );
    }
    std::fs::remove_file(path).ok();
}

#[test]
fn normal16_keeps_a_slope_8_bits_round_flat() {
    // 0.5 + 1/1024 is a quarter of an 8-bit step: `normal` rounds it to 128,
    // `normal16` keeps it to 1/65535.
    let img = Image::filled(2, [0.5 + 1.0 / 1024.0, 0.5, 1.0, 1.0]);
    assert_eq!(encode(&img, Slot::Normal).get_pixel(0, 0).0[0], 128);
    let path = rusty_test_support::temp::dir().join("rusty_bake_normal16_2002.png");
    bake_to_png(&img, Slot::Normal16, path.to_str().unwrap()).unwrap();
    let back = image::open(&path).unwrap();
    assert_eq!(back.color(), image::ColorType::Rgba16);
    let r = f32::from(back.to_rgba16().get_pixel(0, 0).0[0]) / 65535.0;
    assert!((r - img.pixels()[0][0]).abs() <= 0.5 / 65535.0, "{r}");
    assert_eq!(
        Slot::parse("normal16").unwrap().material_maps(),
        ["normal_map"]
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn height_bakes_its_red_at_16_bits_into_the_height_map() {
    let img = Image::filled(2, [0.25 + 1.0 / 1024.0, 0.9, 0.1, 0.0]);
    let path = rusty_test_support::temp::dir().join("rusty_bake_height_2191.png");
    bake_to_png(&img, Slot::Height, path.to_str().unwrap()).unwrap();
    let back = image::open(&path).unwrap();
    assert_eq!(
        back.color(),
        image::ColorType::Rgba16,
        "height bakes 16-bit"
    );
    let texel = back.to_rgba16().get_pixel(0, 0).0;
    let red = f32::from(texel[0]) / 65535.0;
    assert!((red - img.pixels()[0][0]).abs() <= 0.5 / 65535.0, "{red}");
    assert_eq!([texel[1], texel[2], texel[3]], [texel[0], texel[0], 65535]);
    assert_eq!(
        Slot::parse("height").unwrap().material_maps(),
        ["height_map"]
    );
    std::fs::remove_file(path).ok();
}

#[test]
fn colour_slots_stay_8_bit() {
    let path = rusty_test_support::temp::dir().join("rusty_bake_colour_8bit_1491.png");
    bake_to_png(&Image::new(2), Slot::BaseColor, path.to_str().unwrap()).unwrap();
    assert_eq!(image::open(&path).unwrap().color(), image::ColorType::Rgba8);
    std::fs::remove_file(path).ok();
}

#[test]
fn linear_to_srgb_endpoints() {
    assert!((linear_to_srgb(0.0)).abs() < 1e-6);
    assert!((linear_to_srgb(1.0) - 1.0).abs() < 1e-6);
}
