//! The motion-vector history and gate (#1435), adapter-free: what last frame a camera
//! and an entity reproject from, when that history is a cut, and when a view writes
//! velocity at all.

use glam::{Mat4, Vec3};

use super::{track, wanted, HistoryKey, MotionHistory};
use crate::components::CameraComponent;
use crate::core::settings::graphics_options::GraphicsOption;
use crate::core::settings::quality::QualityPreset;
use crate::core::settings::GraphicsSettings;
use crate::render::postfx::params::PostSource;
use crate::render::InstanceData;
use crate::scene::SceneId;

pub(super) fn key(scene: SceneId, base: Option<u32>) -> HistoryKey {
    HistoryKey {
        scene,
        base,
        playing: false,
    }
}

/// A one-camera stack: camera entity 1 through `m`.
pub(super) fn one(m: Mat4) -> [(Option<u32>, Mat4); 1] {
    [(Some(1), m)]
}

fn at(x: f32) -> Mat4 {
    Mat4::from_translation(Vec3::X * x)
}

#[test]
fn a_camera_reprojects_through_last_frame_and_a_cut_forgets_it() {
    let (scene, mut h) = (SceneId::next(), MotionHistory::default());
    assert_eq!(
        h.begin(key(scene, Some(1)), &one(at(1.0))),
        [at(1.0)],
        "first frame"
    );
    h.end();
    assert_eq!(h.begin(key(scene, Some(1)), &one(at(2.0))), [at(1.0)]);
    h.end();
    // A camera switch, then a scene load: no smear from the old matrix.
    assert_eq!(h.begin(key(scene, Some(2)), &one(at(3.0))), [at(3.0)]);
    h.end();
    assert_eq!(
        h.begin(key(SceneId::next(), Some(2)), &one(at(4.0))),
        [at(4.0)]
    );
}

#[test]
fn an_entity_has_last_frames_pose_only_if_it_was_drawn_then() {
    let (k, mut h) = (key(SceneId::next(), None), MotionHistory::default());
    h.begin(k, &one(Mat4::IDENTITY));
    h.record(7, at(1.0), &[]);
    assert!(
        h.pose(7).is_none(),
        "this frame's pose is next frame's history"
    );
    h.end();
    h.begin(k, &one(Mat4::IDENTITY));
    assert_eq!(h.pose(7).map(|p| p.model), Some(at(1.0)));
    h.end();
    h.begin(k, &one(Mat4::IDENTITY));
    assert!(h.pose(7).is_none(), "not drawn last frame: no history");
    h.clear();
    assert!(h.pose(7).is_none());
}

#[test]
fn track_gives_an_instance_its_last_pose_and_palette() {
    let (k, mut h) = (key(SceneId::next(), None), MotionHistory::default());
    let (mut joints, mut first) = (Vec::new(), InstanceData::IDENTITY);
    let rig = [at(1.0), at(2.0)];
    h.begin(k, &one(Mat4::IDENTITY));
    track(&mut h, 3, (at(5.0), &rig), 1, &mut joints, &mut first);
    assert_eq!(first.motion, 0, "a first draw moves with the camera only");
    h.end();

    h.begin(k, &one(Mat4::IDENTITY));
    let (mut joints, mut second) = (vec![[0.0; 16]; 2], InstanceData::IDENTITY);
    track(&mut h, 3, (at(6.0), &rig), 1, &mut joints, &mut second);
    assert_eq!(second.motion, 1);
    assert_eq!(second.prev_model_matrix, at(5.0).to_cols_array());
    assert_eq!(
        second.prev_bone_base, 3,
        "last palette appended after this one"
    );
    assert_eq!(joints[2], at(1.0).to_cols_array());
    h.end();

    // The skin changed size: this frame's palette under last frame's matrix.
    h.begin(k, &one(Mat4::IDENTITY));
    let mut third = InstanceData::IDENTITY;
    track(
        &mut h,
        3,
        (at(7.0), &rig[..1]),
        9,
        &mut Vec::new(),
        &mut third,
    );
    assert_eq!((third.motion, third.prev_bone_base), (1, 9));
}

#[test]
fn velocity_is_written_only_where_motion_blur_runs() {
    fn post(camera: &CameraComponent) -> PostSource<'_> {
        PostSource {
            camera: Some(camera),
            volume: None,
        }
    }
    let mut cam = CameraComponent {
        motion_blur_active: true,
        ..CameraComponent::default()
    };
    assert!(wanted(post(&cam), QualityPreset::Medium.into(), true));
    assert!(
        !wanted(post(&cam), QualityPreset::Low.into(), true),
        "Low pays nothing"
    );
    assert!(
        !wanted(post(&cam), QualityPreset::High.into(), false),
        "no full post-FX"
    );
    assert!(!wanted(
        PostSource::default(),
        QualityPreset::High.into(),
        true
    ));
    let mut off = GraphicsSettings::from(QualityPreset::High);
    off.options.set(GraphicsOption::MotionBlur, false);
    assert!(!wanted(post(&cam), off, true), "the player turned it off");
    cam.motion_blur_active = false;
    assert!(!wanted(post(&cam), QualityPreset::High.into(), true));
}

#[test]
fn an_instance_carries_its_last_pose_in_80_bytes() {
    assert_eq!(std::mem::size_of::<InstanceData>(), 240 + 80);
}
