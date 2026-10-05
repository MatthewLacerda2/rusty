//! Shapes that fold or curve (#831): still no overlap, and the same bits every time.

use super::super::unwrap;
use super::{assert_valid, on, spiral_ramp};

#[test]
fn a_spiral_ramp_never_folds_onto_itself() {
    let (positions, indices) = spiral_ramp();
    assert_valid(&positions, &unwrap(&positions, &indices, &on()).unwrap());
}

#[test]
fn a_sphere_unwraps_without_overlap() {
    let (vertices, indices) = crate::components::mesh::primitives::generate_sphere(1.0, 8, 8);
    // The primitive already carries its unwrap; re-unwrap the raw positions.
    let positions: Vec<[f32; 3]> = vertices.iter().map(|v| v.position).collect();
    assert_valid(&positions, &unwrap(&positions, &indices, &on()).unwrap());
}

#[test]
fn the_same_mesh_unwraps_bit_for_bit_the_same() {
    let (positions, indices) = spiral_ramp();
    let a = unwrap(&positions, &indices, &on()).unwrap();
    assert_eq!(Some(a), unwrap(&positions, &indices, &on()));
}
