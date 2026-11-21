//! A mirrored mesh's front faces and an unmirrored one's back faces are the same cull state, so every pass needs three pipelines, not six. Which side of a kept triangle is shaded is the fragment shader's.

use glam::Mat4;

use super::LazyPipeline;
use rusty_components::{MaterialAsset, RenderFace};

pub(crate) fn is_mirrored(world: Mat4) -> bool {
    world.determinant() < 0.0
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum Facing {
    #[default]
    Ccw,
    Cw,
    Both,
}

impl Facing {
    pub(crate) const ALL: [Self; 3] = [Self::Ccw, Self::Cw, Self::Both];

    pub(crate) fn of(world: Mat4, face: RenderFace) -> Self {
        match face {
            RenderFace::Both => Self::Both,
            RenderFace::Front if is_mirrored(world) => Self::Cw,
            RenderFace::Front => Self::Ccw,
            RenderFace::Back if is_mirrored(world) => Self::Ccw,
            RenderFace::Back => Self::Cw,
        }
    }

    pub(crate) fn of_material(world: Mat4, material: Option<&MaterialAsset>) -> Self {
        Self::of(world, material.map_or(RenderFace::Front, |m| m.render_face))
    }

    pub(crate) fn front_face(self) -> wgpu::FrontFace {
        match self {
            Self::Cw => wgpu::FrontFace::Cw,
            Self::Ccw | Self::Both => wgpu::FrontFace::Ccw,
        }
    }

    pub(crate) fn cull_mode(self) -> Option<wgpu::Face> {
        match self {
            Self::Ccw | Self::Cw => Some(wgpu::Face::Back),
            Self::Both => None,
        }
    }

    pub(crate) fn primitive(self, primitive: wgpu::PrimitiveState) -> wgpu::PrimitiveState {
        wgpu::PrimitiveState {
            front_face: self.front_face(),
            cull_mode: self.cull_mode(),
            ..primitive
        }
    }
}

pub(crate) struct FacedPipeline([LazyPipeline; 3]);

impl FacedPipeline {
    pub(crate) fn new(make: impl Fn(Facing) -> LazyPipeline) -> Self {
        Self(Facing::ALL.map(make))
    }

    pub(crate) fn lazy<B>(
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        layout: &wgpu::PipelineLayout,
        build: B,
    ) -> Self
    where
        B: Fn(
                &wgpu::Device,
                &wgpu::ShaderModule,
                &wgpu::PipelineLayout,
                Facing,
            ) -> wgpu::RenderPipeline
            + Clone
            + Send
            + Sync
            + 'static,
    {
        Self::new(|facing| {
            let (shader, layout, build) = (shader.clone(), layout.clone(), build.clone());
            LazyPipeline::new(device, move |device| {
                build(device, &shader, &layout, facing)
            })
        })
    }

    pub(crate) fn eager(make: impl Fn(Facing) -> LazyPipeline) -> Self {
        let set = Self::new(make);
        set.get(Facing::Ccw);
        set
    }

    pub(crate) fn get(&self, facing: Facing) -> &wgpu::RenderPipeline {
        self.0[facing as usize].get()
    }

    #[cfg(test)]
    pub(crate) fn is_built(&self, facing: Facing) -> bool {
        self.0[facing as usize].is_built()
    }

    pub(crate) fn lazies(&self) -> impl Iterator<Item = &LazyPipeline> {
        self.0.iter()
    }

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
