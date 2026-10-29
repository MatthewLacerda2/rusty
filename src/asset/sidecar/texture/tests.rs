use super::*;

#[test]
fn settings_round_trip_through_the_sidecar_and_a_missing_one_is_the_default() {
    let dir = crate::test_temp::dir();
    let png = dir.join("brick.png");
    assert_eq!(texture(&png), TextureSettings::default());
    let set = TextureSettings {
        wrap_mode: WrapMode::Mirror,
        filter_mode: FilterMode::Point,
        generate_mipmaps: false,
        texture_type: TextureType::SingleChannel,
        compression: Compression::HighQuality,
        max_size: 512,
    };
    set_texture(&png, set).unwrap();
    assert_eq!(texture(&png), set);
    assert!(load(&png).unwrap().colliders.is_empty(), "the rest kept");
}

#[test]
fn names_parse_back_and_an_unknown_one_lists_the_known() {
    for m in WrapMode::ALL {
        assert_eq!(WrapMode::parse(m.name()), Ok(m));
    }
    for m in FilterMode::ALL {
        assert_eq!(FilterMode::parse(m.name()), Ok(m));
    }
    for c in Compression::ALL {
        assert_eq!(Compression::parse(c.name()), Ok(c));
    }
    for t in TextureType::ALL {
        assert_eq!(TextureType::parse(t.name()), Ok(t));
    }
    assert_eq!(parse_max_size(1024), Ok(1024));
    assert!(parse_max_size(1000).unwrap_err().contains("2048, 4096"));
    let err = FilterMode::parse("Linear").unwrap_err();
    assert!(
        err.contains("\"Linear\"") && err.contains("Trilinear"),
        "{err}"
    );
}

#[test]
fn a_max_size_off_the_list_in_a_sidecar_reads_as_the_default() {
    let dir = crate::test_temp::dir();
    let png = dir.join("odd.png");
    std::fs::write(meta_path(&png), r#"{"texture": {"max_size": 1000}}"#).unwrap();
    assert_eq!(texture(&png).max_size, DEFAULT_MAX_SIZE);
}

#[test]
fn a_default_image_in_the_normal_slot_becomes_a_normal_map_and_others_stay() {
    let dir = crate::test_temp::dir();
    let png = dir.join("bumps.png");
    image::RgbaImage::new(4, 4).save(&png).unwrap();
    fit_normal_slot(&png);
    assert_eq!(texture(&png).texture_type, TextureType::NormalMap);
    let single = dir.join("rough.png");
    image::RgbaImage::new(4, 4).save(&single).unwrap();
    let set = TextureSettings {
        texture_type: TextureType::SingleChannel,
        ..Default::default()
    };
    set_texture(&single, set).unwrap();
    fit_normal_slot(&single);
    assert_eq!(texture(&single), set, "a type set by hand is kept");
    fit_normal_slot(&dir.join("missing.png"));
    assert!(
        !meta_path(&dir.join("missing.png")).exists(),
        "no file, no sidecar"
    );
}
