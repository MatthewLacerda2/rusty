use glam::{Mat4, Vec3};

// Camera representation
pub struct Camera {
    pub position: Vec3,
    pub yaw: f32,   // Degrees (rotation around Y axis)
    pub pitch: f32, // Degrees (rotation around X axis)
    pub fov: f32,   // Vertical field of view, in degrees
}

/// Default vertical field of view, in degrees.
pub const DEFAULT_FOV: f32 = 45.0;

impl Camera {
    pub fn new(position: Vec3, yaw: f32, pitch: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            fov: DEFAULT_FOV,
        }
    }

    pub fn forward(&self) -> Vec3 {
        let pitch_rad = self.pitch.to_radians();
        let yaw_rad = self.yaw.to_radians();

        let cos_pitch = pitch_rad.cos();
        let sin_pitch = pitch_rad.sin();
        let cos_yaw = yaw_rad.cos();
        let sin_yaw = yaw_rad.sin();

        Vec3::new(cos_yaw * cos_pitch, sin_pitch, sin_yaw * cos_pitch).normalize()
    }

    pub fn right(&self) -> Vec3 {
        self.forward().cross(Vec3::Y).normalize()
    }

    pub fn build_view_projection(&self, aspect: f32) -> Mat4 {
        let forward = self.forward();
        let view = Mat4::look_at_rh(self.position, self.position + forward, Vec3::Y);
        let proj = Mat4::perspective_rh(self.fov.to_radians(), aspect, 0.1, 200.0);
        proj * view
    }
}
