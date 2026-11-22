use glam::{Vec3, Vec4};

use super::super::raster::{dilate, fill_rest, Texel};

pub(super) type Kept = (Vec<Vec3>, Option<Vec<Vec4>>, Option<Vec<Vec4>>);

/// Rings of empty texels dilation fills around each chart.
pub(super) const DILATE_PASSES: u32 = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Lightmap {
    pub entity: u32,
    pub size: u32,
    filled: Vec<u32>,
    texels: Vec<Vec3>,
    directions: Option<Vec<Vec4>>,
    masks: Option<Vec<Vec4>>,
}

impl Lightmap {
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

    pub fn texels(&self) -> Vec<Vec3> {
        self.square(&self.texels, Vec3::ZERO)
    }

    pub fn directions(&self) -> Vec<Vec4> {
        let square = |kept: &Vec<Vec4>| self.square(kept, Vec4::ZERO);
        self.directions.as_ref().map(square).unwrap_or_default()
    }

    pub fn masks(&self) -> Vec<Vec4> {
        let square = |kept: &Vec<Vec4>| self.square(kept, Vec4::ONE);
        self.masks.as_ref().map(square).unwrap_or_default()
    }

    pub fn is_directional(&self) -> bool {
        self.directions.is_some()
    }

    pub fn is_masked(&self) -> bool {
        self.masks.is_some()
    }

    pub fn covered(&self) -> usize {
        self.filled.len()
    }

    pub fn kept_bytes(&self) -> usize {
        let maps = [&self.directions, &self.masks].map(|m| m.as_ref().map_or(0, Vec::len));
        self.filled.len() * (4 + 12) + (maps[0] + maps[1]) * 16
    }

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
