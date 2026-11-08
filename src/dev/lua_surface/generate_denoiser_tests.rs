//! `Lighting.SetSettings{ denoiser = … }` (#1884): any case of a denoiser's name is
//! taken, and an unknown one raises and changes nothing.

use super::super::super::session::Session;
use crate::scene::lighting::lightmap::Denoiser;

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
        (Denoiser::Filter, 32),
        "nothing changed"
    );
}
