//! A bake's atlas layout before any texel is drawn, and its pages composed one at a time from it, so a bake never holds them all.

use glam::{Vec3, Vec4};

use super::{layout, Cell, PAD};
use crate::lightmap::bake::Lightmap;

pub fn packing(maps: &[Lightmap], max_page: u32) -> Packing {
    let l = layout(maps, max_page);
    Packing {
        page_size: l.page_size,
        page_count: l.page_count,
        directional: !maps.is_empty() && maps.iter().all(Lightmap::is_directional),
        masked: !maps.is_empty() && maps.iter().all(Lightmap::is_masked),
        sizes: l.sizes,
        corners: l.corners,
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Packing {
    pub page_size: u32,
    pub page_count: usize,
    pub directional: bool,
    pub masked: bool,
    sizes: Vec<u32>,
    corners: Vec<(u32, u32, u32)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct AtlasPage {
    pub texels: Vec<Vec3>,
    pub directions: Option<Vec<Vec4>>,
    pub masks: Option<Vec<Vec4>>,
}

impl Packing {
    pub fn on_page(&self, page: usize) -> impl Iterator<Item = usize> + '_ {
        (0..self.corners.len()).filter(move |&i| self.corners[i].0 as usize == page)
    }

    pub fn page<'a>(&self, page: usize, map: &dyn Fn(usize) -> &'a Lightmap) -> AtlasPage {
        let area = (self.page_size * self.page_size) as usize;
        let mut texels = vec![Vec3::ZERO; area];
        let mut directions = self.directional.then(|| vec![Vec4::ZERO; area]);
        // A mask's background is lit: only a chart's own texels hold a shadow.
        let mut masks = self.masked.then(|| vec![Vec4::ONE; area]);
        for i in self.on_page(page) {
            let (_, x, y) = self.corners[i];
            let at = Cell {
                page_size: self.page_size,
                size: self.sizes[i],
                x,
                y,
            };
            let m = map(i);
            at.blit(&mut texels, &m.texels(), m.size);
            if let Some(page) = &mut directions {
                at.blit(page, &m.directions(), m.size);
            }
            if let Some(page) = &mut masks {
                at.blit(page, &m.masks(), m.size);
            }
        }
        AtlasPage {
            texels,
            directions,
            masks,
        }
    }

    pub fn placements(&self, maps: &[Lightmap]) -> Vec<(u32, u32, [f32; 4])> {
        let entities = maps.iter().map(|m| m.entity);
        let edge = self.page_size as f32;
        let place = |(entity, (&(page, x, y), &size)): (u32, (&(u32, u32, u32), &u32))| {
            let scale = size as f32 / edge;
            let (u, v) = ((x + PAD) as f32 / edge, (y + PAD) as f32 / edge);
            (entity, page, [scale, scale, u, v])
        };
        entities
            .zip(self.corners.iter().zip(&self.sizes))
            .map(place)
            .collect()
    }
}
