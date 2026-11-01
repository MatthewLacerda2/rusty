// engine/shaders/lighting/sh.wgsl — the real L2 spherical-harmonic basis (#1675),
// `scene::Sh9`'s order and constants, shared by every shader that evaluates SH: the
// probes' `forward::lighting::eval_sh` and the `Skybox` ambient's
// `lighting::ambient::sky_sh`. One copy, so the two can never disagree.

#define_import_path lighting::sh

// The nine basis functions at unit direction `n`.
fn sh_basis(n: vec3<f32>) -> array<f32, 9> {
    var b: array<f32, 9>;
    b[0] = 0.2820948;
    b[1] = -0.4886025 * n.y;
    b[2] = 0.4886025 * n.z;
    b[3] = -0.4886025 * n.x;
    b[4] = 1.0925484 * n.x * n.y;
    b[5] = -1.0925484 * n.y * n.z;
    b[6] = 0.31539157 * (3.0 * n.z * n.z - 1.0);
    b[7] = -1.0925484 * n.x * n.z;
    b[8] = 0.5462742 * (n.x * n.x - n.y * n.y);
    return b;
}
