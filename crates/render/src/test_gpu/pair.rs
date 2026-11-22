use crate::Renderer;

pub fn headless_pair_or_skip(width: u32, height: u32) -> Option<(Renderer, HeadlessPair)> {
    let pair = headless_pair();
    let first = pair.build(width, height)?;
    Some((first, pair))
}

pub fn headless_pair() -> HeadlessPair {
    HeadlessPair(crate::setup::budget::acquire_headless_pair())
}

pub struct HeadlessPair(crate::setup::budget::Pair);

impl HeadlessPair {
    pub fn renderer(&self, width: u32, height: u32) -> Renderer {
        self.build(width, height)
            .expect("the pair's first renderer found an adapter")
    }

    fn build(&self, width: u32, height: u32) -> Option<Renderer> {
        pollster::block_on(Renderer::new_headless_with(self.0.permit(), width, height))
    }
}
