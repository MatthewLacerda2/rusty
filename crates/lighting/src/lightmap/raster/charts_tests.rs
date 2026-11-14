//! A lightmap's charts and the in-chart fill (#1949).

use glam::{Vec2, Vec3};

use super::charts::{chart_of_triangles, fill_within};
use super::Surface;

#[test]
fn triangles_sharing_a_uv_corner_are_one_chart() {
    // Two quads apart in UV, each two triangles, and the second quad's vertices
    // duplicated (a seam): still two charts.
    let uv = [(0.0, 0.0), (0.4, 0.0), (0.4, 1.0), (0.0, 1.0)];
    let mut uvs: Vec<Vec2> = uv.iter().map(|&(u, v)| Vec2::new(u, v)).collect();
    uvs.extend(uv.iter().map(|&(u, v)| Vec2::new(u + 0.6, v)));
    uvs.extend([Vec2::new(0.6, 0.0), Vec2::new(1.0, 1.0)]);
    let positions = vec![Vec3::ZERO; uvs.len()];
    let indices = [0, 1, 2, 0, 2, 3, 4, 5, 6, 8, 9, 7];
    let surface = Surface {
        positions: &positions,
        normals: &positions,
        uvs: &uvs,
        indices: &indices,
    };
    assert_eq!(chart_of_triangles(&surface), vec![0, 0, 1, 1]);
}

#[test]
fn an_invalid_texel_takes_only_its_own_charts_light() {
    // A 2×2 map: chart 0 on the first row (texel 1 invalid), chart 1, dark, on the
    // second, so the invalid texel neighbours chart 1 as much as chart 0.
    let mut texels = [Vec3::ONE, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO];
    let mut filled = [true, false, true, true];
    let labels = [0, 0, 1, 1];
    assert_eq!(fill_within(&mut texels, &mut filled, &labels, 2), 0);
    assert_eq!(texels[1], Vec3::ONE, "filled from chart 0 only");
    assert!(filled.iter().all(|&f| f));
}

#[test]
fn a_cut_off_texel_takes_its_charts_mean_and_a_lost_chart_is_counted() {
    // 3×3: chart 0 at texels 0 and 8 (opposite corners; 8 invalid, cut off by
    // chart 1 between them), chart 2 at texel 4, wholly invalid. The rest empty.
    let labels = [0, 1, 1, 1, 2, 1, 1, 1, 0];
    let mut texels = [Vec3::splat(2.0); 9];
    texels[8] = Vec3::ZERO;
    let mut filled = [true, true, true, true, false, true, true, true, false];
    let lost = fill_within(&mut texels, &mut filled, &labels, 3);
    assert_eq!(lost, 1, "chart 2 has no valid texel");
    assert_eq!(texels[8], Vec3::splat(2.0), "chart 0's mean");
    assert!(!filled[4], "a lost chart is left to dilation");
}
