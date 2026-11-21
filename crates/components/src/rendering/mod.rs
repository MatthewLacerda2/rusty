pub mod camera;
pub mod light;
pub mod light_probe_proxy_volume;
pub mod lod_group;
pub mod post_processing;

impl crate::FirstClass for camera::CameraComponent {
    const NAME: &'static str = "Camera";
}
