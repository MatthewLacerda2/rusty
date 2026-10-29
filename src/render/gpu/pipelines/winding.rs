//! Which way round a placed mesh's triangles face (#1134, #1142), and which faces a
//! culling pipeline keeps. A world matrix with a negative determinant mirrors what
//! it places and reverses its winding, so a pipeline that culls by facing needs a
//! twin that takes clockwise triangles as front; a two-sided shadow caster (#1571)
//! needs one that culls nothing. The shadow casters pick theirs here.

use glam::Mat4;

/// Whether `world` mirrors what it places: a negative determinant reverses a
/// mesh's winding, so face culling would keep the wrong side.
pub(crate) fn is_mirrored(world: Mat4) -> bool {
    world.determinant() < 0.0
}

/// Which faces of a placed mesh a pipeline draws. Order is draw order: plain
/// casters first, so the twins bind last.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Facing {
    /// Counter-clockwise front faces, back faces culled: the usual case.
    Front,
    /// A mirrored mesh's front faces, which wind clockwise (#1134).
    Mirrored,
    /// Every face, nothing culled: a two-sided caster (#1571), mirrored or not.
    Both,
}

impl Facing {
    /// The facing a mesh placed by `world` is drawn with; `two_sided` draws both.
    pub(crate) fn of(world: Mat4, two_sided: bool) -> Self {
        match (two_sided, is_mirrored(world)) {
            (true, _) => Self::Both,
            (false, true) => Self::Mirrored,
            (false, false) => Self::Front,
        }
    }

    /// The winding the pipeline takes as front.
    pub(crate) fn front_face(self) -> wgpu::FrontFace {
        match self {
            Self::Mirrored => wgpu::FrontFace::Cw,
            Self::Front | Self::Both => wgpu::FrontFace::Ccw,
        }
    }

    /// The faces the pipeline culls.
    pub(crate) fn cull_mode(self) -> Option<wgpu::Face> {
        match self {
            Self::Front | Self::Mirrored => Some(wgpu::Face::Back),
            Self::Both => None,
        }
    }
}
