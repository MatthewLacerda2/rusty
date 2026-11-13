//! The loading budget is Unity's per priority, `BelowNormal` by default (#2123).

use super::LoadingPriority;
use crate::application::Application;

#[test]
fn the_budget_is_unitys_and_below_normal_by_default() {
    let ms = LoadingPriority::ALL.map(|p| p.budget().as_millis());
    assert_eq!(ms, [2, 4, 10, 50]);
    let app = Application::new();
    assert_eq!(
        app.background_loading_priority(),
        LoadingPriority::BelowNormal
    );
}

#[test]
fn a_priority_reads_back_by_its_name_and_an_unknown_one_is_none() {
    for p in LoadingPriority::ALL {
        assert_eq!(LoadingPriority::parse(p.name()), Some(p));
    }
    assert_eq!(LoadingPriority::parse("Highest"), None);
    let mut app = Application::new();
    app.set_background_loading_priority(LoadingPriority::High);
    assert_eq!(app.background_loading_priority(), LoadingPriority::High);
}
