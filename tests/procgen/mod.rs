mod bake;
mod bands;
mod hard_surface;
mod image_input;
mod ops;
mod organic;
mod resolution;
mod seams;
mod stones;
mod weathering;

use rusty::procgen::recipe::{Constant, Node, OpKind, TextureRecipe};
use rusty::procgen::{evaluate, Image};

/// Node `id` running `op` over the nodes named in `inputs`.
pub fn node(id: &str, op: OpKind, inputs: &[&str]) -> Node {
    Node {
        id: id.into(),
        op,
        inputs: inputs.iter().map(|s| s.to_string()).collect(),
    }
}

/// A flat-colour node.
pub fn constant(id: &str, c: [f32; 4]) -> Node {
    node(id, OpKind::Constant(Constant { color: c }), &[])
}

/// Evaluate `nodes` at `resolution` with `seed`; the last node is the output.
pub fn bake(nodes: Vec<Node>, resolution: u32, seed: u64) -> Image {
    let recipe = TextureRecipe::new(resolution, nodes).with_seed(seed);
    evaluate(&recipe).expect("recipe evaluates")
}

/// Evaluate `nodes` at `resolution`, reading the node named `output`.
pub fn run(nodes: Vec<Node>, output: &str, resolution: u32) -> Image {
    let recipe = TextureRecipe::new(resolution, nodes).with_output(output);
    evaluate(&recipe).expect("recipe evaluates")
}

/// [`bake`] at 64 px.
pub fn bake_seeded(nodes: &[Node], seed: u64) -> Image {
    bake(nodes.to_vec(), 64, seed)
}
