use super::{tile_size, tiles, BlurParams};
use crate::components::CameraComponent;

fn camera() -> CameraComponent {
    CameraComponent {
        motion_blur_active: true,
        motion_blur_samples: 16,
        ..CameraComponent::default()
    }
}

#[test]
fn tiles_cover_half_the_longest_streak_in_steps_of_eight() {
    assert_eq!(tile_size(0.0), 8, "a tile is never empty");
    assert_eq!(tile_size(16.0), 8);
    assert_eq!(tile_size(17.0), 16, "a 8.5 px reach needs the next step");
    // 1080p at the default clamp: a 54 px streak, 27 px either side.
    assert_eq!(tile_size(0.05 * 1080.0), 32);
    assert_eq!((tiles(1920, 32), tiles(1080, 32), tiles(0, 8)), (60, 34, 1));
}

#[test]
fn the_camera_knobs_land_in_the_uniform() {
    let p = BlurParams::new(&camera(), 1080);
    assert_eq!(p.shape, [0.5, 54.0, 16.0, 32.0]);
    assert_eq!(p.tile(), 32);
}

#[test]
fn out_of_range_knobs_are_held_to_what_the_renderer_draws() {
    let cam = CameraComponent {
        motion_blur_samples: 99,
        motion_blur_intensity: 4.0,
        motion_blur_max_length: 1.0,
        ..camera()
    };
    let p = BlurParams::new(&cam, 100);
    assert_eq!(p.shape, [1.0, 20.0, 32.0, 16.0]);
}

#[test]
fn the_sky_distance_is_one_number_in_rust_and_wgsl() {
    let wgsl = include_str!("../../../../engine/shaders/sky/motion.wgsl");
    let line = format!(
        "const SKY_DISTANCE: f32 = {:.1};",
        crate::render::motion::SKY_DISTANCE
    );
    assert!(
        wgsl.contains(&line),
        "sky/motion.wgsl should declare `{line}`"
    );
}

#[test]
fn velocities_over_several_frames_shrink_the_shutter_not_the_clamp() {
    let one = BlurParams::new(&camera(), 1080);
    let four = one.per_frames(4);
    assert_eq!(four.shape, [0.125, 54.0, 16.0, 32.0]);
    assert_eq!(one.per_frames(0), one, "0 frames reads as one");
}
