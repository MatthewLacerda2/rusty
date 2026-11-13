//! crates/physics/src/shape/decompose/fingerprint_tests.rs — what makes a decomposition
//! is pinned in `core::hulls::DECOMPOSER`, which every hull sidecar carries
//! (#1940). A change here fails until the fingerprint is bumped, so a sidecar
//! another decomposer wrote is refused, not simulated.

use rapier3d::parry::transformation::vhacd::VHACDParameters;

use crate::shape::hull::SLACK;
use rusty_core::hulls::{DECOMPOSER, PARRY};

/// `VHACDParameters::default()` when `DECOMPOSER` was last bumped.
const PARAMETERS: &str = "VHACDParameters { concavity: 0.01, alpha: 0.05, \
    beta: 0.05, resolution: 64, plane_downsampling: 4, convex_hull_downsampling: 4, \
    fill_mode: FloodFill { detect_cavities: false }, convex_hull_approximation: true, \
    max_convex_hulls: 1024 }";

#[test]
fn the_vhacd_parameters_and_the_piece_filter_are_the_fingerprinted_ones() {
    let bump = "bump core::hulls::DECOMPOSER, then this pin";
    let now = format!("{:?}", VHACDParameters::default());
    assert_eq!(now, PARAMETERS, "V-HACD's parameters changed: {bump}");
    assert_eq!(SLACK, 1e-5, "has_volume's slack changed: {bump}");
    assert!(DECOMPOSER.contains("has-volume-1e-5"), "{bump}");
}

#[test]
fn the_fingerprint_follows_the_locked_parry() {
    let lock = std::fs::read_to_string(rusty_core::project::checkout().join("Cargo.lock"))
        .expect("Cargo.lock");
    let locked = lock
        .split("[[package]]")
        .find(|p| p.contains("\nname = \"parry3d\"\n"))
        .and_then(|p| p.lines().find_map(|l| l.strip_prefix("version = ")))
        .map(|v| v.trim_matches('"').to_string())
        .expect("parry3d in Cargo.lock");
    assert_eq!(
        locked, PARRY,
        "parry changed: bump core::hulls::DECOMPOSER and PARRY"
    );
    assert!(
        DECOMPOSER.ends_with(&format!("parry3d-{PARRY}")),
        "{DECOMPOSER}"
    );
}
