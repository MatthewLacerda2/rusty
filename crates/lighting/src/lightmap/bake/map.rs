//! One mesh's baked lightmap as a finished bake keeps it (#2312): only the
//! texels its charts cover, never the empty rest of its `size`² square. A mesh
//! whose UV islands cover a sliver of their square (a long thin trim, small
//! islands far apart) would otherwise keep ten times its traced texels until the
//! pages are written. The square is filled again ([`Lightmap::texels`]) when its
//! page is composed: the chart's edge dilated and the rest filled from the
//! nearest chart, the same functions on the same texels, so the page's bytes are
//! the ones a dense map gave.

use glam::{Vec3, Vec4};

use super::super::raster::{dilate, fill_rest, Texel};

/// A sparse map's kept texels: their light, and their directions and masks
/// when the bake makes those.
pub(super) type Kept = (Vec<Vec3>, Option<Vec<Vec4>>, Option<Vec<Vec4>>);

/// Rings of empty texels dilation fills around each chart.
pub(super) const DILATE_PASSES: u32 = 3;

/// One mesh's baked lightmap: `size`² linear texels, row-major, each the `E / π`
/// value the forward shader multiplies by albedo (see `trace`), kept as the
/// texels its charts cover ([`Lightmap::covered`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Lightmap {
    pub entity: u32,
    pub size: u32,
    /// The row-major index of every texel kept, ascending: its charts' texels.
    filled: Vec<u32>,
    /// Per kept texel, its light.
    texels: Vec<Vec3>,
    /// Per kept texel (#810), `xyz` the dominant direction its light arrives from,
    /// scaled by its directionality (length 1 when it all comes from one way, 0
    /// when it comes evenly from everywhere), and `w` its lamp share (#1695: how
    /// much of it is `Baked` lamps' direct light from one way,
    /// `direction::direction_texel`). Empty when the bake was not
    /// [`BakeSettings::directional`](rusty_scene::lighting::settings::BakeSettings::directional).
    directions: Option<Vec<Vec4>>,
    /// Per kept texel (#1695), its shadowmask: per channel the static occlusion of
    /// the `Mixed` light on it (`trace::Tracer::mask`). Empty unless the bake gave
    /// a light a channel (`BakeScene::shadowmask`).
    masks: Option<Vec<Vec4>>,
}

impl Lightmap {
    /// A lightmap of whole `size`² maps, every texel kept: `directions` and
    /// `masks` either as many or empty.
    pub fn dense(
        entity: u32,
        size: u32,
        texels: Vec<Vec3>,
        directions: Vec<Vec4>,
        masks: Vec<Vec4>,
    ) -> Self {
        let filled = (0..texels.len() as u32).collect();
        let some = |v: Vec<Vec4>| (!v.is_empty()).then_some(v);
        Self::sparse(
            entity,
            size,
            filled,
            (texels, some(directions), some(masks)),
        )
    }

    /// A lightmap keeping the texels at `filled` (row-major indices into its
    /// `size`² square, ascending), each with its light, and its direction and
    /// mask when the bake makes those maps.
    pub(super) fn sparse(
        entity: u32,
        size: u32,
        filled: Vec<u32>,
        (texels, directions, masks): Kept,
    ) -> Self {
        debug_assert_eq!(filled.len(), texels.len());
        let one_each = |v: &Option<Vec<Vec4>>| v.as_ref().is_none_or(|v| v.len() == texels.len());
        debug_assert!(one_each(&directions) && one_each(&masks));
        Self {
            entity,
            size,
            filled,
            texels,
            directions,
            masks,
        }
    }

    /// The `size`² lightmap, row-major: the kept texels, their edge dilated and
    /// the rest of the square filled from the nearest one.
    pub fn texels(&self) -> Vec<Vec3> {
        self.square(&self.texels, Vec3::ZERO)
    }

    /// The `size`² direction map ([`Self::texels`]' layout); empty when the bake
    /// was not directional.
    pub fn directions(&self) -> Vec<Vec4> {
        let square = |kept: &Vec<Vec4>| self.square(kept, Vec4::ZERO);
        self.directions.as_ref().map(square).unwrap_or_default()
    }

    /// The `size`² shadowmask ([`Self::texels`]' layout); empty unless the bake
    /// gave a light a channel.
    pub fn masks(&self) -> Vec<Vec4> {
        let square = |kept: &Vec<Vec4>| self.square(kept, Vec4::ONE);
        self.masks.as_ref().map(square).unwrap_or_default()
    }

    /// Whether it carries a direction map.
    pub fn is_directional(&self) -> bool {
        self.directions.is_some()
    }

    /// Whether it carries a shadowmask.
    pub fn is_masked(&self) -> bool {
        self.masks.is_some()
    }

    /// How many texels of the square it keeps: its charts'.
    pub fn covered(&self) -> usize {
        self.filled.len()
    }

    /// The bytes it keeps: its index, light, direction and mask per kept texel.
    pub fn kept_bytes(&self) -> usize {
        let maps = [&self.directions, &self.masks].map(|m| m.as_ref().map_or(0, Vec::len));
        self.filled.len() * (4 + 12) + (maps[0] + maps[1]) * 16
    }

    /// `kept`, one per kept texel, as the whole square over `background`: the
    /// chart's edge dilated ([`DILATE_PASSES`]) and the rest filled from the
    /// nearest kept texel.
    fn square<T: Texel>(&self, kept: &[T], background: T) -> Vec<T> {
        let area = (self.size * self.size) as usize;
        let mut map = vec![background; area];
        let mut filled = vec![false; area];
        for (&at, &value) in self.filled.iter().zip(kept) {
            map[at as usize] = value;
            filled[at as usize] = true;
        }
        if self.filled.len() < area {
            dilate(&mut map, &mut filled, self.size, DILATE_PASSES);
            fill_rest(&mut map, &filled, self.size);
        }
        map
    }
}
