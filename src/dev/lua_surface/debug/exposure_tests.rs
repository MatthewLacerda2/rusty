//! `Debug.GetAdaptedExposure` (#1444): the screen's adapted EV a frontend publishes,
//! `nil` before one has.

use crate::dev::session::Session;

#[test]
fn reads_the_published_adapted_exposure_and_nil_before_it() {
    let s = Session::new("").expect("an empty session boots");
    assert_eq!(
        s.eval("return tostring(Debug.GetAdaptedExposure())")
            .unwrap(),
        "nil"
    );
    s.world().scene().borrow_mut().screen_exposure = Some(2.5);
    assert_eq!(s.eval("return Debug.GetAdaptedExposure()").unwrap(), "2.5");
}
