use super::{cube, load, strip, Lut};

/// An identity `.cube` of `n` a side, as Resolve writes one.
pub(crate) fn identity_cube(n: u32) -> String {
    cube_of(n, |rgb| rgb)
}

/// A `.cube` of `n` a side mapping each input `[r, g, b]` through `f`.
pub(crate) fn cube_of(n: u32, f: impl Fn([f32; 3]) -> [f32; 3]) -> String {
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

#[test]
fn an_identity_cube_reads_red_fastest() {
    let lut = cube::parse(&identity_cube(3)).unwrap();
    assert_eq!((lut.size, lut.domain), (3, [[0.0; 3], [1.0; 3]]));
    assert_eq!(lut.texels.len(), 27);
    assert_eq!(lut.texels[1], [0.5, 0.0, 0.0, 1.0], "red grows first");
    assert_eq!(lut.texels[3], [0.0, 0.5, 0.0, 1.0]);
    assert_eq!(lut.texels[9], [0.0, 0.0, 0.5, 1.0]);
}

#[test]
fn domain_keywords_set_the_input_range() {
    let text = "DOMAIN_MIN 0 0 0\nDOMAIN_MAX 2 2 4\n".to_string() + &identity_cube(2);
    assert_eq!(
        cube::parse(&text).unwrap().domain,
        [[0.0; 3], [2.0, 2.0, 4.0]]
    );
    let text = "LUT_3D_INPUT_RANGE -1 1\n".to_string() + &identity_cube(2);
    assert_eq!(cube::parse(&text).unwrap().domain, [[-1.0; 3], [1.0; 3]]);
}

#[test]
fn a_wrong_entry_count_names_the_size_line() {
    let mut text = identity_cube(4);
    text.truncate(text.trim_end().rfind('\n').unwrap());
    let (line, why) = cube::parse(&text).unwrap_err();
    assert_eq!(line, 3, "{why}");
    assert!(why.contains("needs 64 entries, found 63"), "{why}");
    let extra = identity_cube(2) + "0 0 0\n";
    assert_eq!(cube::parse(&extra).unwrap_err().0, 13);
}

#[test]
fn malformed_lines_are_refused_with_their_line() {
    let bad = |text: &str| cube::parse(text).unwrap_err();
    assert_eq!(bad("LUT_3D_SIZE 2\n0 0\n").0, 2);
    assert_eq!(bad("LUT_3D_SIZE 2\n0 x 0\n").0, 2);
    assert_eq!(bad("LUT_3D_SIZE 2\n0 inf 0\n").0, 2);
    assert_eq!(bad("0 0 0\nLUT_3D_SIZE 2\n").0, 1, "data before the size");
    assert_eq!(bad("LUT_1D_SIZE 16\n").0, 1);
    assert_eq!(bad("LUT_3D_SIZE 99\n").0, 1, "past MAX_SIZE");
    assert_eq!(bad("LUT_3D_SIZE 1\n").0, 1, "under MIN_SIZE");
    assert!(bad("# nothing\n").1.contains("no LUT_3D_SIZE"));
    let flipped = "DOMAIN_MIN 1 1 1\nDOMAIN_MAX 0 0 0\n".to_string() + &identity_cube(2);
    assert!(bad(&flipped).1.contains("DOMAIN_MAX"));
    let late = identity_cube(2) + "TITLE \"late\"\n";
    assert!(bad(&late).1.contains("after the table"));
}

#[test]
fn a_strip_reads_blue_by_slice_and_green_upwards() {
    let n = 4u32;
    let w = n * n;
    let mut rgba = vec![0u8; (w * n * 4) as usize];
    // The texel (r 1, g 0, b 2): slice 2, column 1, the bottom row.
    let at = (((n - 1) * w + 2 * n + 1) * 4) as usize;
    rgba[at..at + 3].copy_from_slice(&[255, 51, 0]);
    let lut = strip::from_rgba8(w, n, &rgba).unwrap();
    assert_eq!(lut.size, n);
    assert_eq!(lut.texels[(1 + 2 * 16) as usize], [1.0, 0.2, 0.0, 1.0]);
    assert!(strip::from_rgba8(300, 20, &rgba)
        .unwrap_err()
        .contains("300×20"));
}

#[test]
fn load_names_the_file_and_line() {
    let dir = crate::test_temp::dir();
    let path = dir.join("short.cube");
    std::fs::write(&path, "LUT_3D_SIZE 2\n0 0 0\n").unwrap();
    let why = load(&path).unwrap_err().to_string();
    assert!(why.contains("short.cube:1: LUT_3D_SIZE 2 needs 8"), "{why}");
    let ok = dir.join("id.CUBE");
    std::fs::write(&ok, identity_cube(2)).unwrap();
    assert_eq!(load(&ok).unwrap().size, Lut::MIN_SIZE);
    assert!(load(&dir.join("x.tga")).is_err());
}
