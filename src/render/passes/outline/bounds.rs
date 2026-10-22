//! Where the selection sits on screen (#1175). The outline's dilation and ring are
//! scissored to it, so their cost follows the selection's size on screen, not the
//! view's.

use glam::{Mat4, Vec2, Vec3};

use crate::render::{MeshId, Renderer};
use crate::scene::Scene;

/// The world space the outline can cover: the ringed solids' bounds, or everywhere
/// when one of them has a pose that cannot be bounded.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum OutlineBounds {
    Box(Vec3, Vec3),
    Everywhere,
}

impl Renderer {
    /// The union of `ids`' world bounds, as the frustum cull bounds them (a skinned
    /// mesh's posed bound, #833); `None` when none of them is resident.
    pub(crate) fn outline_bounds(&self, scene: &Scene, ids: &[u32]) -> Option<OutlineBounds> {
        ids.iter()
            .filter_map(|&id| {
                let mesh = scene.world.mesh(id)?;
                let gpu = self.gpu_meshes.get(&MeshId::from_mesh(&mesh))?;
                let world = scene.world.local_to_world(id);
                Some(match gpu.world_bounds(&mesh, world) {
                    Some((min, max)) => OutlineBounds::Box(min, max),
                    None => OutlineBounds::Everywhere,
                })
            })
            .reduce(OutlineBounds::union)
    }
}

impl OutlineBounds {
    fn union(self, other: Self) -> Self {
        match (self, other) {
            (Self::Box(a, b), Self::Box(c, d)) => Self::Box(a.min(c), b.max(d)),
            _ => Self::Everywhere,
        }
    }

    /// The pixel rectangle `[x, y, width, height]` these bounds cover in a `size`
    /// view through `view_proj`, grown by `pad` pixels and clamped to the view;
    /// `None` when it lies wholly outside. Bounds reaching behind the camera, or
    /// `Everywhere`, cover the whole view.
    pub(crate) fn screen_rect(
        self,
        view_proj: Mat4,
        size: (u32, u32),
        pad: u32,
    ) -> Option<[u32; 4]> {
        let (w, h) = (size.0 as f32, size.1 as f32);
        let whole = Some([0, 0, size.0, size.1]);
        let Self::Box(min, max) = self else {
            return whole;
        };
        let (mut lo, mut hi) = (Vec2::splat(f32::MAX), Vec2::splat(f32::MIN));
        for i in 0..8 {
            let pick = |bit: usize, a: f32, b: f32| if i & bit == 0 { a } else { b };
            let corner = Vec3::new(
                pick(1, min.x, max.x),
                pick(2, min.y, max.y),
                pick(4, min.z, max.z),
            );
            let clip = view_proj * corner.extend(1.0);
            if clip.w <= 1e-4 {
                return whole;
            }
            let ndc = clip.truncate().truncate() / clip.w;
            let px = Vec2::new((ndc.x * 0.5 + 0.5) * w, (0.5 - ndc.y * 0.5) * h);
            (lo, hi) = (lo.min(px), hi.max(px));
        }
        let pad = pad as f32;
        let (x0, y0) = ((lo.x - pad).floor().max(0.0), (lo.y - pad).floor().max(0.0));
        let (x1, y1) = ((hi.x + pad).ceil().min(w), (hi.y + pad).ceil().min(h));
        (x1 > x0 && y1 > y0).then_some([x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32])
    }
}
