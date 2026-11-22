use super::{default_drift, serde_drift, FirstClass};
use serde::{Deserialize, Serialize};

// The named rows' Add Component defaults are scene builders, checked in `scene::create::defaults`.
macro_rules! drift_of {
    (collider, $ty:path) => {
        Vec::<String>::new()
    };
    (light, $ty:path) => {
        Vec::<String>::new()
    };
    (material, $ty:path) => {
        Vec::<String>::new()
    };
    (mesh, $ty:path) => {
        Vec::<String>::new()
    };
    (rigidbody, $ty:path) => {
        Vec::<String>::new()
    };
    ($field:ident, $ty:path) => {
        default_drift(&<$ty>::default())
    };
}

macro_rules! check_rows {
    ($($field:ident: $ty:path => $kind:ident [$($acc:ident)*];)*) => {
        vec![$((stringify!($field), drift_of!($field, $ty)),)*]
    };
}

#[test]
fn a_missing_key_loads_the_components_default() {
    let drift: Vec<_> = crate::first_class!(check_rows)
        .into_iter()
        .filter(|(_, paths)| !paths.is_empty())
        .collect();
    assert!(
        drift.is_empty(),
        "a serde default differs from the component's `Default`, so a scene missing \
         that key loads another value; make the serde default return what `Default` \
         sets: {drift:#?}"
    );
}

#[derive(Serialize, Deserialize)]
struct Drifting {
    #[serde(default = "one")]
    kept: u32,
    #[serde(default)]
    agreed: u32,
}

impl Default for Drifting {
    fn default() -> Self {
        Self { kept: 2, agreed: 0 }
    }
}

impl FirstClass for Drifting {
    const NAME: &'static str = "Drifting";
}

fn one() -> u32 {
    1
}

#[test]
fn a_serde_default_that_differs_from_default_is_flagged() {
    let drift = default_drift(&Drifting::default());
    assert_eq!(drift, [r#"Drifting: kept (loads {"agreed":0,"kept":1})"#]);
}

#[derive(Default, Serialize, Deserialize)]
struct Agreeing {
    #[serde(default)]
    inner: Nested,
    required: u32,
}

#[derive(Serialize, Deserialize)]
struct Nested {
    #[serde(default = "three")]
    deep: u32,
    #[serde(default = "three")]
    drifts: u32,
}

impl Default for Nested {
    fn default() -> Self {
        Self { deep: 3, drifts: 3 }
    }
}

fn three() -> u32 {
    3
}

#[test]
fn defaults_that_agree_and_required_keys_pass() {
    assert!(serde_drift(&Agreeing::default()).is_empty());
}

#[test]
fn a_nested_key_is_dropped_on_its_own() {
    let mut drifted = Agreeing::default();
    drifted.inner.drifts = 4;
    let paths: Vec<_> = serde_drift(&drifted).into_iter().map(|(p, _)| p).collect();
    assert_eq!(
        paths,
        ["inner", "inner.drifts"],
        "the object, then its key alone"
    );
}

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Stale(u32);

impl FirstClass for Stale {
    const NAME: &'static str = "Stale";
    const DEFAULT_EXCEPTIONS: &'static [(&'static str, &'static str)] = &[("gone", "a test")];
}

#[derive(Default, Serialize, Deserialize)]
struct Drops {
    #[serde(default = "three")]
    kept: u32,
}

impl FirstClass for Drops {
    const NAME: &'static str = "Drops";
    const DEFAULT_EXCEPTIONS: &'static [(&'static str, &'static str)] = &[("kept", "a test")];
}

#[test]
fn an_exception_hides_its_drift_and_a_stale_one_is_flagged() {
    assert!(default_drift(&Drops::default()).is_empty());
    assert_eq!(
        default_drift(&Stale(0)),
        ["Stale: gone is excepted but loads its default"]
    );
}
