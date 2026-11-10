//! `Lighting.SetSettings{ denoiser = … }` (#1884): any case of a denoiser's name is
//! taken, and an unknown one raises and changes nothing; and `backfaceTolerance`
//! (#1949), clamped like the other knobs.

use super::super::super::session::Session;
use crate::scene::lighting::lightmap::{BakeSettings, Denoiser};

#[test]
fn the_denoiser_sets_by_name_and_refuses_an_unknown_one() {
    let s = Session::new("").expect("an empty session boots");
    let denoiser = || {
        s.world()
            .scene()
            .borrow()
            .lighting_settings
            .lightmaps
            .denoiser
    };
    assert_eq!(denoiser(), Denoiser::Oidn);
    s.eval("Lighting.SetSettings{ denoiser = 'filter' }")
        .unwrap();
    assert_eq!(denoiser(), Denoiser::Filter);
    let read = "return Lighting.GetSettings().denoiser";
    assert_eq!(s.eval(read).unwrap(), "Filter");
    let err = s.eval("Lighting.SetSettings{ denoiser = 'OptiX', samples = 3 }");
    assert!(err.unwrap_err().contains("OIDN, Filter or None"));
    let samples = s
        .world()
        .scene()
        .borrow()
        .lighting_settings
        .lightmaps
        .samples;
    assert_eq!(
        (denoiser(), samples),
        (Denoiser::Filter, BakeSettings::DEFAULT_SAMPLES),
        "nothing changed"
    );
}

#[test]
fn backface_tolerance_reads_back_and_clamps() {
    let s = Session::new("").expect("an empty session boots");
    let read = "return Lighting.GetSettings().backfaceTolerance";
    let default = BakeSettings::DEFAULT_BACKFACE_TOLERANCE;
    assert_eq!(s.eval(read).unwrap().parse::<f32>().unwrap(), default);
    let tolerance = |s: &Session| s.world().scene().borrow().lighting_settings.lightmaps;
    s.eval("Lighting.SetSettings{ backfaceTolerance = 0.5 }")
        .unwrap();
    assert_eq!(tolerance(&s).backface_tolerance, 0.5);
    s.eval("Lighting.SetSettings{ backfaceTolerance = 3 }")
        .unwrap();
    assert_eq!(
        tolerance(&s).backface_tolerance,
        1.0,
        "clamped to Unity's 0–1"
    );
    s.eval("Lighting.SetSettings{ backfaceTolerance = 0/0 }")
        .unwrap();
    assert_eq!(
        tolerance(&s).backface_tolerance,
        default,
        "NaN takes the default"
    );
}
