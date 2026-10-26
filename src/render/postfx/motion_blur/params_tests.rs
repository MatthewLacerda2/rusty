use glam::{Mat4, Vec3};

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
    let p = BlurParams::new(&camera(), 1080, Mat4::IDENTITY);
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
    let p = BlurParams::new(&cam, 100, Mat4::IDENTITY);
    assert_eq!(p.shape, [1.0, 20.0, 32.0, 16.0]);
}

#[test]
fn the_depth_rows_give_back_the_distance_in_front_of_the_camera() {
    let projection = crate::scene::Camera::new(Vec3::ZERO, 0.0, 0.0).projection_matrix(1.5);
    let p = BlurParams::new(&camera(), 100, projection);
    for distance in [0.5, 7.0, 150.0] {
        let clip = projection * Vec3::new(0.3, -0.2, -distance).extend(1.0);
        let d = clip.z / clip.w;
        let [x, y, z, w] = p.depth;
        let got = -(x * d + y) / (z * d + w);
        assert!(
            (got - distance).abs() < distance * 1e-3,
            "{got} vs {distance}"
        );
    }
}
