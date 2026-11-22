//! Packing and lazy loading change memory, never bytes. A grayscale image (every pixel's R, G and B the
//! same bits, alpha exactly 1) is stored as its red channel, a quarter of the RGBA
//! f32 buffer, and written back out bit for bit when a consumer reads it. A file a
//! loader decodes is kept only from the first `image` node that reads it to the
//! last.

use std::borrow::Cow;

use crate::image_buf::{Image, Size};
use crate::recipe::{Fault, NodeError, OpKind, TextureRecipe};
use crate::source::Sources;
use rusty_core::collections::Map;

pub(super) enum Held {
    Rgba(Image),
    Gray(Size, Vec<f32>),
}

impl Held {
    pub fn pack(img: Image) -> Held {
        let one = 1.0f32.to_bits();
        let gray = img.pixels().iter().all(|p| {
            let r = p[0].to_bits();
            p[1].to_bits() == r && p[2].to_bits() == r && p[3].to_bits() == one
        });
        if !gray {
            return Held::Rgba(img);
        }
        Held::Gray(img.size(), img.pixels().iter().map(|p| p[0]).collect())
    }

    pub fn view(&self) -> Cow<'_, Image> {
        match self {
            Held::Rgba(img) => Cow::Borrowed(img),
            Held::Gray(size, red) => Cow::Owned(gray_image(*size, red)),
        }
    }

    pub fn bytes(&self) -> usize {
        match self {
            Held::Rgba(img) => super::slots::image_bytes(img),
            Held::Gray(_, red) => std::mem::size_of_val(red.as_slice()),
        }
    }

    pub fn into_image(self) -> Image {
        match self {
            Held::Rgba(img) => img,
            Held::Gray(size, red) => gray_image(size, &red),
        }
    }
}

fn gray_image(size: Size, red: &[f32]) -> Image {
    let mut img = Image::new(size);
    img.map_indexed(|i, _| [red[i], red[i], red[i], 1.0]);
    img
}

pub(super) struct Resident {
    files: Sources,
    readers: Map<String, usize>,
}

impl Resident {
    pub fn new(recipe: &TextureRecipe, order: &[usize], sources: &Sources) -> Self {
        let mut readers: Map<String, usize> = Map::default();
        for &i in order {
            if let OpKind::Image(p) = &recipe.nodes[i].op {
                if sources.get(&p.path).is_none() {
                    *readers.entry(p.path.clone()).or_default() += 1;
                }
            }
        }
        Self {
            files: Sources::new(),
            readers,
        }
    }

    pub fn for_node<'s>(
        &'s mut self,
        recipe: &TextureRecipe,
        i: usize,
        sources: &'s Sources,
    ) -> Result<&'s Sources, NodeError> {
        let node = &recipe.nodes[i];
        let OpKind::Image(p) = &node.op else {
            return Ok(sources);
        };
        if !self.readers.contains_key(&p.path) {
            return Ok(sources);
        }
        if self.files.get(&p.path).is_none() {
            let fault = |error| Fault::Unreadable {
                path: p.path.clone(),
                error,
            };
            let loaded = sources
                .load(&p.path)
                .unwrap_or_else(|| Err("the loader does not know this path".into()));
            self.files
                .insert(&p.path, loaded.map_err(|e| node.error(fault(e)))?);
        }
        Ok(&self.files)
    }

    pub fn done(&mut self, recipe: &TextureRecipe, i: usize) {
        let OpKind::Image(p) = &recipe.nodes[i].op else {
            return;
        };
        if let Some(n) = self.readers.get_mut(&p.path) {
            *n -= 1;
            if *n == 0 {
                self.readers.remove(&p.path);
                self.files.remove(&p.path);
            }
        }
    }
}
