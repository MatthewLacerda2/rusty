use super::*;
use crate::core::settings::project_graphics::ProjectGraphics;

#[test]
fn names_round_trip_and_a_bad_one_is_refused() {
    for upscaler in Upscaler::ALL {
        assert_eq!(Upscaler::parse(upscaler.name()), Ok(upscaler));
    }
    assert!(Upscaler::parse("dlss").is_err());
    assert!(Upscaler::parse("FSR2").is_err(), "names are lowercase");
    assert_eq!(Upscaler::default(), Upscaler::Taa);
}

#[test]
fn the_project_default_runs_while_storage_holds_no_choice() {
    let mut storage = Storage::new();
    let setting = UpscalerSetting::load(&storage, Upscaler::Fsr2);
    assert_eq!(setting.resolve(), Upscaler::Fsr2);
    // A stored player choice overrides it.
    Upscaler::Taa.store(&mut storage);
    let setting = UpscalerSetting::load(&storage, Upscaler::Fsr2);
    assert_eq!(setting.resolve(), Upscaler::Taa);
    // Forgetting the choice deletes the key, so the default is back next launch.
    let forgot = UpscalerSetting {
        chosen: None,
        ..setting
    };
    forgot.store(&mut storage);
    assert!(storage.get(GRAPHICS_NAMESPACE, UPSCALER_KEY).is_none());
    assert_eq!(
        UpscalerSetting::load(&storage, Upscaler::Fsr2).resolve(),
        Upscaler::Fsr2
    );
    // A hand-edited value that names no upscaler is no choice.
    storage.set(GRAPHICS_NAMESPACE, UPSCALER_KEY, serde_json::json!("xess"));
    assert_eq!(Upscaler::load(&storage), None);
}

#[test]
fn the_graphics_block_reads_an_upscaler() {
    let read = |json: &str| ProjectGraphics::read_json(&serde_json::from_str(json).unwrap());
    assert_eq!(read(r#"{"upscaler": "fsr2"}"#).0.upscaler, Upscaler::Fsr2);
    assert_eq!(read("{}").0.upscaler, Upscaler::Taa);
    for bad in [r#""dlss""#, "1", "null"] {
        let (graphics, ignored) = read(&format!(r#"{{"upscaler": {bad}}}"#));
        assert_eq!(graphics.upscaler, Upscaler::Taa, "{bad} keeps taa");
        assert_eq!(ignored.len(), 1, "{bad} is warned about");
    }
}

#[test]
fn the_upscaler_round_trips_through_the_project_file() {
    use crate::core::project::ProjectFile;
    let mut file = ProjectFile::default();
    file.graphics.upscaler = Upscaler::Fsr2;
    let text = file.to_json();
    assert!(text.contains(r#""upscaler": "fsr2""#), "{text}");
    assert_eq!(
        ProjectFile::from_json(&text).unwrap().graphics.upscaler,
        Upscaler::Fsr2
    );
}
