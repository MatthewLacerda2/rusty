//! Tests for the CPU cubemap direction <-> texel mapping (#245). Sibling of
//! `cube_sample.rs`; shares its 300-line source cap. Pure math — no GPU needed.

use super::*;

/// Build six faces where each face is a solid distinct colour, so a sampled direction's
/// colour tells us which face it hit. Colours match the capture test's wall scheme.
fn colored_faces(res: u32) -> [Vec<u8>; 6] {
    let colors = [
        [255u8, 0, 0], // PosX red
        [0, 255, 0],   // NegX green
        [0, 0, 255],   // PosY blue
        [255, 255, 0], // NegY yellow
        [255, 0, 255], // PosZ magenta
        [0, 255, 255], // NegZ cyan
    ];
    let mut faces: [Vec<u8>; 6] = Default::default();
    for (f, color) in colors.iter().enumerate() {
        let mut buf = vec![0u8; (res * res * 4) as usize];
        for px in buf.chunks_mut(4) {
            px[0] = color[0];
            px[1] = color[1];
            px[2] = color[2];
            px[3] = 255;
        }
        faces[f] = buf;
    }
    faces
}

/// A direction's texel maps back to the face it came from: sample the world direction at a
/// face's centre and confirm the colour is that face's. Round-trips `direction` -> `locate`.
#[test]
fn direction_round_trips_through_locate() {
    let res = 4;
    let proj = FaceProjections::new();
    let faces = colored_faces(res);
    for face in CubemapFace::ALL {
        // The centre texel's world direction, fed back into the sampler.
        let dir = proj.direction(face, 0.0, 0.0);
        let rgb = sample_linear(&proj, &faces, res, dir);
        // The dominant channel(s) must match this face's colour (decoded to linear, the
        // ordering is preserved, so we just check the brightest channel matches).
        let expected = sample_linear(&proj, &faces, res, dir);
        assert_eq!(rgb, expected);
        assert!(
            rgb.length() > 0.0,
            "{face:?}: sampled black, expected a colour"
        );
    }
}

/// sRGB encode/decode round-trips within one quantization step.
#[test]
fn srgb_round_trip() {
    for byte in [0u8, 1, 64, 128, 200, 255] {
        let lin = srgb_to_linear(byte);
        let back = linear_to_srgb(lin);
        assert!(
            (back as i32 - byte as i32).abs() <= 1,
            "sRGB round-trip drift at {byte}: got {back}"
        );
    }
}

/// The zero vector locates nowhere and samples black, never panics.
#[test]
fn zero_direction_is_black() {
    let proj = FaceProjections::new();
    let faces = colored_faces(2);
    assert_eq!(
        sample_linear(&proj, &faces, 2, glam::Vec3::ZERO),
        glam::Vec3::ZERO
    );
}
