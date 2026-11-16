//! A thin bright line's reflection holds under TAA's jitter (#2228): a mirror
//! bronze ceiling reflecting a long LED strip about a pixel tall on screen, which
//! hangs 1.5 m in front of a dark wall, the camera held still, TAA's sub-pixel
//! jitter cycled. A march probing only each step's end, several texels apart,
//! landed on such a line on some offsets and stepped over it on others: stretches
//! of its reflection went missing on 8 of the jitter's 27 offsets (on `tower-bar`,
//! a dark patch blinking in a satin soffit). The min-depth pyramid
//! (`ssr_march.wgsl`'s `clear`) probes every texel a step passes in front of.

use glam::Vec3;

use super::super::fixture::{paint, place, Surface};
use crate::readback::read_texture_bytes;
use crate::{RenderView, Renderer, OFFSCREEN_FORMAT};
use rusty_components::{CameraComponent, MaterialAsset, MaterialComponent};
use rusty_core::color::f16_to_f32;
use rusty_scene::create::{create_entity, default_post_processing, Primitive};
use rusty_scene::{Camera, Scene};

const W: u32 = 640;
const H: u32 = 360;
const FOV: f32 = 40.0;
/// Frames before the measure; then one cycle of the jitter's vertical (base 3)
/// part, the period the dropout kept.
const WARM: u32 = 4;
const FRAMES: u32 = 27;
/// The rows of the frame the ceiling's reflection of the line lies in.
const CEILING: u32 = H * 6 / 10;

/// A box named `name` centred at `at`, `size` across, painted `surface`.
fn block(s: &mut Scene, name: &str, at: Vec3, size: Vec3, surface: Surface) {
    let id = create_entity(s, name, Some(Primitive::Box));
    place(s, id, at, size);
    paint(s, id, name, surface);
}

/// `rough::outline`'s room under a mirror ceiling, its uprights gone: a glowing
/// strip `tall` metres high and 4 cm deep, 2.5 m up and 1.5 m in front of the back
/// wall; TAA on, the sky black.
fn strip(tall: f32) -> Scene {
    let mut s = Scene::new();
    crate::test_gpu::black_sky(&mut s);
    let cam = s.add_entity("Camera".to_string());
    let camera = CameraComponent {
        taa_active: true,
        fxaa_active: false,
        fov: FOV,
        ..Default::default()
    };
    s.world.set_camera(cam, Some(camera));
    let mut volume = default_post_processing();
    (volume.ssr.active, volume.ssr.min_smoothness) = (true, 0.3);
    (volume.bloom.active, volume.ssao.active) = (false, false);
    s.world.set_post_processing(cam, Some(volume));
    let mirror = ([0.8, 0.55, 0.35], 1.0, 0.04);
    let dark = ([0.05; 3], 0.0, 1.0);
    let room = [
        (
            "ceiling",
            Vec3::new(0.0, 3.45, -3.0),
            [12.0, 0.1, 12.0],
            mirror,
        ),
        (
            "floor",
            Vec3::new(0.0, -0.05, -3.0),
            [12.0, 0.1, 12.0],
            dark,
        ),
        ("wall", Vec3::new(0.0, 1.7, -4.05), [12.0, 3.4, 0.1], dark),
        ("strip", Vec3::new(0.0, 2.5, -2.5), [12.0, tall, 0.04], dark),
    ];
    for (name, at, size, surface) in room {
        block(&mut s, name, at, size.into(), surface);
    }
    let led = MaterialAsset {
        emissive: [24.0, 20.0, 14.0],
        ..s.materials["strip"].clone()
    };
    s.materials.insert("led".into(), led);
    let id = s.find_entity_by_name("strip").unwrap();
    s.world
        .set_material(id, Some(MaterialComponent::new("led".to_string())));
    s
}

/// The eye: 1.6 m up, looking at the back wall, pitched up 30°.
fn eye() -> Camera {
    let mut c = Camera::new(Vec3::new(0.0, 1.6, 0.0), -90.0, 30.0);
    c.fov = FOV;
    c
}

/// Each frame of a jitter cycle after [`WARM`], as perceptual luma per pixel: the
/// trace's hits (what each offset found, before the resolve) and TAA's output.
fn cycle(r: &mut Renderer, scene: &Scene) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H);
    let (mut hits, mut taa) = (Vec::new(), Vec::new());
    for f in 0..WARM + FRAMES {
        let out = v.color_target_view().unwrap();
        r.render(&mut v, scene, &eye(), &out, false);
        if f >= WARM {
            hits.push(luma(r, v.post_fx.ssr_hits().expect("SSR ran")));
            taa.push(luma(r, v.post_fx.taa_latest().expect("TAA ran")));
        }
    }
    (hits, taa)
}

/// `texture` (`Rgba16Float`, the case's size) as perceptual luma (`y / (1 + y)`,
/// in 255ths) per pixel, row by row.
fn luma(r: &Renderer, texture: &wgpu::Texture) -> Vec<f32> {
    let bytes = read_texture_bytes(&r.device, &r.queue, texture, (W, H), 8);
    let half = |i: usize| f16_to_f32(u16::from_le_bytes([bytes[i], bytes[i + 1]])).max(0.0);
    let y = |t: usize| 0.25 * half(t * 8) + 0.5 * half(t * 8 + 2) + 0.25 * half(t * 8 + 4);
    (0..(W * H) as usize)
        .map(|t| 255.0 * y(t) / (1.0 + y(t)))
        .collect()
}

/// A tile's side, in pixels.
const TILE: u32 = 16;

/// The largest one-frame dip of any tile over the [`CEILING`] (the lower of its
/// two neighbours in time, minus it): the measure the patch on `tower-bar` was
/// found by.
fn dip(frames: &[Vec<f32>]) -> f32 {
    let tile = |f: &[f32], t: u32| {
        let (tx, ty) = (t % (W / TILE) * TILE, t / (W / TILE) * TILE);
        let px = (0..TILE * TILE).map(|k| f[((ty + k / TILE) * W + tx + k % TILE) as usize]);
        px.sum::<f32>() / (TILE * TILE) as f32
    };
    let tiles: Vec<Vec<f32>> = frames
        .iter()
        .map(|f| {
            (0..W / TILE * (CEILING / TILE))
                .map(|t| tile(f, t))
                .collect()
        })
        .collect();
    let dips = tiles
        .windows(3)
        .flat_map(|w| (0..w[0].len()).map(move |t| w[0][t].min(w[2][t]) - w[1][t]));
    dips.fold(0.0, f32::max)
}

/// How many rows deep, on average over the columns it spans, the line's
/// reflection is in the trace's hits.
fn depth(hits: &[Vec<f32>]) -> f32 {
    let lit = |f: &Vec<f32>| {
        f[..(W * CEILING) as usize]
            .iter()
            .filter(|&&v| v > 100.0)
            .count()
    };
    let columns = (0..W)
        .filter(|&x| {
            hits.iter()
                .any(|f| (0..CEILING).any(|y| f[(y * W + x) as usize] > 100.0))
        })
        .count();
    hits.iter().map(lit).sum::<usize>() as f32 / (hits.len() * columns.max(1)) as f32
}

/// The most a tile of the trace's hits may dip between two offsets: one row of
/// its 16 at the line's brightness (about 240), the line's own raster edge
/// moving under the jitter. Measured: 15.2 and 15.2 (lavapipe); `main`,
/// which lost stretches, 31.0 and 30.3.
const HITS_DIP: f32 = 18.0;
/// The most a tile of TAA's output may dip, for the 12 mm line: the 6 mm one is
/// under a pixel tall, gone from the frame itself on some offsets, so TAA's
/// output dips on its own aliasing as `main`'s did (2.4; `main` 1.6). Measured:
/// 1.2 (lavapipe); `main` 13.9.
const TAA_DIP: f32 = 5.0;
/// The most rows deep the line's reflection may be, on average: a ray passing
/// behind a line hits it within 1% of its depth (`deep`), not
/// `THICKNESS`'s 15 cm. Measured: 4.4 and 4.5 (lavapipe); `main`, its curves of
/// stray hits, 7.1 for the 6 mm line.
const ROWS: f32 = 6.0;

#[test]
fn gpu_a_thin_bright_lines_reflection_holds_under_taa_jitter() {
    let Some(mut r) = crate::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    for tall in [0.006, 0.012] {
        let (hits, taa) = cycle(&mut r, &strip(tall));
        let (traced, seen, rows) = (dip(&hits), dip(&taa), depth(&hits));
        eprintln!("[ssr] a {tall} m line over a jitter cycle: the trace's tiles dip {traced:.1}, TAA's {seen:.1}; its reflection is {rows:.1} rows deep");
        assert!(
            traced <= HITS_DIP,
            "a {tall} m line's reflection loses a stretch under the jitter: a tile dips {traced:.1}"
        );
        assert!(
            tall < 0.01 || seen <= TAA_DIP,
            "a {tall} m line's reflection blinks after TAA: a tile dips {seen:.1}"
        );
        assert!(
            rows <= ROWS,
            "a {tall} m line reflects {rows:.1} rows deep: a thin object over-reflects"
        );
    }
}
