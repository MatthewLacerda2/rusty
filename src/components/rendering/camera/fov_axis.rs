//! src/components/rendering/camera/fov_axis.rs — which axis the editor and scripts
//! show a camera's field of view along (#2038, Unity's `Camera.FieldOfViewAxis`),
//! and Unity's two conversions between them.
//!
//! The stored `fov` is always the **vertical** angle, as Unity's
//! `Camera.fieldOfView` is: the axis only changes how the Inspector shows and
//! edits it (Unity's `CameraEditor` converts at the view's aspect), so flipping it
//! never changes what the camera draws.

crate::core::named_enum! {
    /// The axis a camera's field of view is shown and edited along.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    pub enum FovAxis {
        /// The view's height spans the angle (the stored value as it is).
        #[default]
        Vertical = "Vertical",
        /// The view's width spans the angle, at the view's aspect.
        Horizontal = "Horizontal",
    }
}

/// The horizontal field of view (degrees) of a `vertical` one at `aspect`
/// (width / height), Unity's `Camera.VerticalToHorizontalFieldOfView`.
pub fn vertical_to_horizontal_fov(vertical: f32, aspect: f32) -> f32 {
    let half = (vertical.to_radians() * 0.5).tan() * aspect;
    2.0 * half.atan().to_degrees()
}

/// The vertical field of view (degrees) of a `horizontal` one at `aspect`
/// (width / height), Unity's `Camera.HorizontalToVerticalFieldOfView`.
pub fn horizontal_to_vertical_fov(horizontal: f32, aspect: f32) -> f32 {
    let half = (horizontal.to_radians() * 0.5).tan() / aspect;
    2.0 * half.atan().to_degrees()
}

impl FovAxis {
    /// The stored vertical `fov` shown along this axis at `aspect`.
    pub fn show(self, vertical: f32, aspect: f32) -> f32 {
        match self {
            Self::Vertical => vertical,
            Self::Horizontal => vertical_to_horizontal_fov(vertical, aspect),
        }
    }

    /// The vertical `fov` to store for `value` read along this axis at `aspect`.
    pub fn store(self, value: f32, aspect: f32) -> f32 {
        match self {
            Self::Vertical => value,
            Self::Horizontal => horizontal_to_vertical_fov(value, aspect),
        }
    }
}
