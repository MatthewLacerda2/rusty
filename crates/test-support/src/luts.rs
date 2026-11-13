//! `.cube` colour lookups as text, for the tests of `asset`'s parser and of the
//! renderer's Color Lookup.

/// An identity `.cube` of `n` a side, as Resolve writes one.
pub fn identity_cube(n: u32) -> String {
    cube_of(n, |rgb| rgb)
}

/// A `.cube` of `n` a side mapping each input `[r, g, b]` through `f`.
pub fn cube_of(n: u32, f: impl Fn([f32; 3]) -> [f32; 3]) -> String {
    let mut text = format!("# a test LUT\nTITLE \"test\"\nLUT_3D_SIZE {n}\n\n");
    let step = |i: u32| i as f32 / (n - 1) as f32;
    for b in 0..n {
        for g in 0..n {
            for r in 0..n {
                let [r, g, b] = f([step(r), step(g), step(b)]);
                text += &format!("{r:.6} {g:.6} {b:.6}\n");
            }
        }
    }
    text
}
