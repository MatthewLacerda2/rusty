//! Which triangles of a placed mesh a pipeline keeps (#1134, #1142, #1571, #1615).
//!
//! A material's *Render Face* (`RenderFace`, URP's `_Cull`) says which faces of the
//! mesh draw: the ones turned toward the viewer, the ones turned away, or both. A
//! world matrix with a negative determinant mirrors what it places and reverses its
//! winding, so on screen the mesh's front faces wind clockwise. The two together
//! leave three cull states, a [`Facing`]: keep the counter-clockwise triangles, keep
//! the clockwise ones, or keep all. A mirrored mesh's front faces and an unmirrored
//! one's back faces are the same state, so every pass needs three pipelines, not
//! six. Which side of a kept triangle is shaded is the fragment shader's (it flips
//! the normal of a face seen from behind), so no pipeline state says it.
//!
//! [`FacedPipeline`] holds one pass's three, each built on its first draw.

use glam::Mat4;

use super::LazyPipeline;
use crate::components::RenderFace;

/// Whether `world` mirrors what it places: a negative determinant reverses a
/// mesh's winding, so face culling would keep the wrong side.
pub(crate) fn is_mirrored(world: Mat4) -> bool {
    world.determinant() < 0.0
}

/// Which triangles a pipeline keeps, by their winding on screen. Order is draw
/// order: the usual state first, so the others bind last.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Facing {
    /// Counter-clockwise triangles, the clockwise culled: a mesh's front faces, the
    /// usual case.
    #[default]
    Ccw,
    /// Clockwise triangles: a mirrored mesh's front faces (#1134), or an unmirrored
    /// one's back faces (`RenderFace::Back`).
    Cw,
    /// Every triangle, nothing culled: `RenderFace::Both`, or a two-sided shadow
    /// caster (#1571).
    Both,
}

impl Facing {
    /// Every state, in [`FacedPipeline`]'s order.
    pub(crate) const ALL: [Self; 3] = [Self::Ccw, Self::Cw, Self::Both];

    /// The state a mesh placed by `world` draws its `face` with.
    pub(crate) fn of(world: Mat4, face: RenderFace) -> Self {
        match face {
            RenderFace::Both => Self::Both,
            RenderFace::Front if is_mirrored(world) => Self::Cw,
            RenderFace::Front => Self::Ccw,
            RenderFace::Back if is_mirrored(world) => Self::Ccw,
            RenderFace::Back => Self::Cw,
        }
    }

    /// The winding the pipeline takes as front.
    pub(crate) fn front_face(self) -> wgpu::FrontFace {
        match self {
            Self::Cw => wgpu::FrontFace::Cw,
            Self::Ccw | Self::Both => wgpu::FrontFace::Ccw,
        }
    }

    /// The faces the pipeline culls.
    pub(crate) fn cull_mode(self) -> Option<wgpu::Face> {
        match self {
            Self::Ccw | Self::Cw => Some(wgpu::Face::Back),
            Self::Both => None,
        }
    }

    /// `primitive` drawing this state's triangles.
    pub(crate) fn primitive(self, primitive: wgpu::PrimitiveState) -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            front_face: self.front_face(),
            cull_mode: self.cull_mode(),
            ..primitive
        }
    }
}

/// One pass's pipeline per [`Facing`], each built on its first draw (#985): a
/// scene with no mirrored or double-sided mesh never builds the other two.
pub(crate) struct FacedPipeline([LazyPipeline; 3]);

impl FacedPipeline {
    /// The set `make(facing)` builds.
    pub(crate) fn new(make: impl Fn(Facing) -> LazyPipeline) -> Self {
        Self(Facing::ALL.map(make))
    }

    /// The set, with the usual [`Facing::Ccw`] one built now: for a pass every
    /// frame draws (the opaque pass, its prepass).
    pub(crate) fn eager(make: impl Fn(Facing) -> LazyPipeline) -> Self {
        let set = Self::new(make);
        set.get(Facing::Ccw);
        set
    }

    /// The pipeline for `facing`.
    pub(crate) fn get(&self, facing: Facing) -> &wgpu::RenderPipeline {
        self.0[facing as usize].get()
    }

    /// Whether `facing`'s pipeline has been built.
    #[cfg(test)]
    pub(crate) fn is_built(&self, facing: Facing) -> bool {
        self.0[facing as usize].is_built()
    }

    /// Every one, for warming.
    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.0.iter()
    }

    /// The [`Facing::Cw`] and [`Facing::Both`] ones: what an [`Self::eager`] set
    /// still has to build.
    pub(crate) fn twins(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.0[1..].iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn a_face_and_a_mirror_pick_one_of_three_states() {
        let flip = Mat4::from_scale(Vec3::new(-1.0, 1.0, 1.0));
        let id = Mat4::IDENTITY;
        assert_eq!(Facing::of(id, RenderFace::Front), Facing::Ccw);
        assert_eq!(Facing::of(flip, RenderFace::Front), Facing::Cw);
        assert_eq!(Facing::of(id, RenderFace::Back), Facing::Cw);
        assert_eq!(Facing::of(flip, RenderFace::Back), Facing::Ccw);
        assert_eq!(Facing::of(id, RenderFace::Both), Facing::Both);
        assert_eq!(Facing::of(flip, RenderFace::Both), Facing::Both);
        assert_eq!(Facing::default(), Facing::Ccw);
    }

    #[test]
    fn each_state_culls_back_faces_of_its_own_winding() {
        assert_eq!(Facing::Ccw.cull_mode(), Some(wgpu::Face::Back));
        assert_eq!(Facing::Ccw.front_face(), wgpu::FrontFace::Ccw);
        assert_eq!(Facing::Cw.front_face(), wgpu::FrontFace::Cw);
        assert_eq!(Facing::Both.cull_mode(), None);
        for (i, facing) in Facing::ALL.into_iter().enumerate() {
            assert_eq!(facing as usize, i);
        }
    }
}
