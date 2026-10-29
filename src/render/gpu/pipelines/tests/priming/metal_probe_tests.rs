//! PROBE (#1525), reverted before readying: which pass makes two Metal frames differ.

use glam::Vec3;

use super::cut_depth_tests::dissolve;
use super::mask_tests::Dissolve;
use super::priming_tests::walls;
use crate::core::settings::quality::QualityPreset;
use crate::render::{readback, RenderView, Renderer, OFFSCREEN_FORMAT};
use crate::scene::{Camera, Scene};

const W: u32 = 288;
const H: u32 = 192;

fn frame(r: &mut Renderer, scene: &Scene, view: Option<&mut RenderView>) -> Vec<u8> {
    r.depth_priming = false;
    let mut fresh = None;
    let view = match view {
        Some(v) => v,
        None => fresh.insert(RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2)),
    };
    let out = view.color_target_view().unwrap();
    let cam = Camera::new(Vec3::new(0.0, 1.5, 3.0), -90.0, -5.0);
    r.render(view, scene, &cam, &out, false);
    let target = view.color_target().unwrap();
    readback::read_texture_rgba8(&r.device, &r.queue, target, W, H)
}

fn diff(a: &[u8], b: &[u8]) -> String {
    let mut n = 0;
    let mut cols = std::collections::BTreeMap::<u32, u32>::new();
    let mut first = Vec::new();
    for (i, (p, q)) in a.chunks(4).zip(b.chunks(4)).enumerate() {
        if p != q {
            n += 1;
            let (x, y) = (i as u32 % W, i as u32 / W);
            *cols.entry(x).or_default() += 1;
            if first.len() < 3 {
                first.push(format!("({x},{y}) {p:?}->{q:?}"));
            }
        }
    }
    let top: Vec<_> = cols
        .iter()
        .filter(|(_, c)| **c > 3)
        .map(|(x, c)| format!("{x}:{c}"))
        .collect();
    format!("{n} px; cols {top:?}; {first:?}")
}

type Tweak = fn(&mut Scene, &mut Renderer);

fn pp(s: &mut Scene, f: impl Fn(&mut crate::components::PostProcessingComponent)) {
    let ids = s.entity_ids();
    for id in ids {
        if let Some(mut c) = s.world.post_processing(id).map(|c| (*c).clone()) {
            f(&mut c);
            s.world.set_post_processing(id, Some(c));
        }
    }
}

#[test]
fn gpu_metal_probe_two_frames() {
    let mask = image::RgbaImage::from_fn(4, 1, |x, _| {
        image::Rgba([if x == 1 || x == 2 { 0 } else { 255 }; 4])
    });
    let d = Dissolve::with_mask("probe", &mask);
    let Some(mut r) = crate::render::test_gpu::headless_or_skip(W, H) else {
        return;
    };
    let variants: [(&str, Tweak, bool); 12] = [
        ("base", |_, _| {}, true),
        ("same_view", |_, _| {}, true),
        ("no_cut", |_, _| {}, false),
        ("no_ssao", |s, _| pp(s, |c| c.ssao.active = false), true),
        ("no_ssr", |s, _| pp(s, |c| c.ssr.active = false), true),
        ("no_bloom", |s, _| pp(s, |c| c.bloom.active = false), true),
        (
            "none_of_3",
            |s, _| {
                pp(s, |c| {
                    c.ssao.active = false;
                    c.ssr.active = false;
                    c.bloom.active = false;
                })
            },
            true,
        ),
        (
            "pp_inactive",
            |s, _| {
                pp(s, |c| {
                    c.active = false;
                    c.ssao.active = false;
                })
            },
            true,
        ),
        ("medium", |_, r| r.set_quality(QualityPreset::Medium), true),
        ("low", |_, r| r.set_quality(QualityPreset::Low), true),
        (
            "low_none",
            |s, r| {
                r.set_quality(QualityPreset::Low);
                pp(s, |c| {
                    c.active = false;
                    c.ssao.active = false;
                })
            },
            false,
        ),
        ("base_again", |_, _| {}, true),
    ];
    let mut report = format!("backend {:?}\n", r.priming_recommended);
    for (name, tweak, cut) in variants {
        r.set_quality(QualityPreset::High);
        let (scene, id) = walls(&d, false);
        if cut {
            dissolve(&scene, id, &d, 0.5);
        }
        tweak(&mut scene.borrow_mut(), &mut r);
        let s = scene.borrow();
        let frames: Vec<Vec<u8>> = if name == "same_view" {
            let mut v = RenderView::offscreen(&r.device, OFFSCREEN_FORMAT, W, H, 2);
            (0..3).map(|_| frame(&mut r, &s, Some(&mut v))).collect()
        } else {
            (0..3).map(|_| frame(&mut r, &s, None)).collect()
        };
        let h = frames[0]
            .iter()
            .fold(0u64, |h, b| h.wrapping_mul(1099511628211) ^ u64::from(*b));
        report += &format!(
            "{name} #{h:x}: 1v2 {} | 2v3 {}\n",
            diff(&frames[0], &frames[1]),
            diff(&frames[1], &frames[2])
        );
    }
    panic!("PROBE REPORT\n{report}");
}
