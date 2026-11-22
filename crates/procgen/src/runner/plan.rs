//! Every node's wiring is checked, not only what the targets reach, before any node runs.

use rusty_core::collections::Map;

use super::RunError;
use crate::recipe::{Fault, Link, Node, OpKind, TextureRecipe};

pub(super) struct Plan {
    pub order: Vec<usize>,
    /// Inputs in node order, then links in `OpKind::links` order: `eval_one` splits there.
    pub inputs: Vec<Vec<usize>>,
    pub uses: Vec<usize>,
    pub targets: Vec<usize>,
}

#[derive(Clone, Copy)]
pub(super) enum Scope {
    Needed,
    Every,
}

pub(super) fn plan(
    recipe: &TextureRecipe,
    targets: &[&str],
    scope: Scope,
) -> Result<Plan, RunError> {
    if recipe.extent().pixels() == 0 {
        return Err(RunError::BadResolution);
    }
    if recipe.nodes.is_empty() {
        return Err(RunError::Empty);
    }
    let mut by_id: Map<&str, usize> = Map::default();
    for (i, n) in recipe.nodes.iter().enumerate() {
        if by_id.insert(n.id.as_str(), i).is_some() {
            return Err(RunError::DuplicateId(n.id.clone()));
        }
    }
    let inputs = wire(recipe, &by_id)?;
    if let Scope::Every = scope {
        // Nothing is kept as a target, but a misspelled `output` or `outputs` entry
        // is still refused, as the bakes refuse it.
        let declared = recipe.output.iter().chain(recipe.outputs.values());
        if let Some(id) = declared
            .into_iter()
            .find(|id| !by_id.contains_key(id.as_str()))
        {
            return Err(RunError::UnknownOutput(id.clone()));
        }
    }
    let targets = targets
        .iter()
        .map(|id| {
            by_id
                .get(id)
                .copied()
                .ok_or_else(|| RunError::UnknownOutput(id.to_string()))
        })
        .collect::<Result<Vec<usize>, _>>()?;
    // The walk over every node only refuses cycles. The order comes from a DFS down
    // from the roots, so each node runs right before the consumer that first needs
    // it: a recipe that lists its leaves first still holds a few images at once,
    // not one per leaf.
    let all: Vec<usize> = (0..recipe.nodes.len()).collect();
    Order::new(recipe, &inputs).visit_all(&all)?;
    let roots = match scope {
        Scope::Needed => targets.clone(),
        Scope::Every => sinks(&inputs),
    };
    let order = Order::new(recipe, &inputs).visit_all(&roots)?;
    let mut uses = vec![0; recipe.nodes.len()];
    for j in order.iter().flat_map(|&i| &inputs[i]) {
        uses[*j] += 1;
    }
    Ok(Plan {
        order,
        inputs,
        uses,
        targets,
    })
}

fn wire(recipe: &TextureRecipe, by_id: &Map<&str, usize>) -> Result<Vec<Vec<usize>>, RunError> {
    let wire_node = |node: &Node| {
        if let OpKind::Graph(g) = &node.op {
            let fault = Fault::Unexpanded(g.path.clone());
            return Err(RunError::Node(node.error(fault)));
        }
        node.check_arity().map_err(RunError::Node)?;
        let index = |input: &String| {
            by_id
                .get(input.as_str())
                .copied()
                .ok_or_else(|| RunError::Node(node.error(Fault::UnknownInput(input.clone()))))
        };
        let link = |(param, link): (&str, &Link)| {
            by_id.get(link.node.as_str()).copied().ok_or_else(|| {
                RunError::Node(node.error(Fault::UnknownLink {
                    param: param.to_string(),
                    link: link.node.clone(),
                }))
            })
        };
        let inputs = node.inputs.iter().map(index);
        inputs
            .chain(node.op.links().into_iter().map(link))
            .collect()
    };
    recipe.nodes.iter().map(wire_node).collect()
}

fn sinks(inputs: &[Vec<usize>]) -> Vec<usize> {
    let mut read = vec![false; inputs.len()];
    for &j in inputs.iter().flatten() {
        read[j] = true;
    }
    (0..inputs.len()).filter(|&i| !read[i]).collect()
}

#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    OnStack,
    Done,
}

struct Order<'a> {
    recipe: &'a TextureRecipe,
    inputs: &'a [Vec<usize>],
    state: Vec<Visit>,
    order: Vec<usize>,
}

impl<'a> Order<'a> {
    fn new(recipe: &'a TextureRecipe, inputs: &'a [Vec<usize>]) -> Self {
        Self {
            recipe,
            inputs,
            state: vec![Visit::Unseen; inputs.len()],
            order: Vec::new(),
        }
    }

    fn visit_all(mut self, roots: &[usize]) -> Result<Vec<usize>, RunError> {
        for &i in roots {
            self.visit(i)?;
        }
        Ok(self.order)
    }

    fn visit(&mut self, i: usize) -> Result<(), RunError> {
        match self.state[i] {
            Visit::Done => return Ok(()),
            Visit::OnStack => return Err(RunError::Node(self.recipe.nodes[i].error(Fault::Cycle))),
            Visit::Unseen => {}
        }
        self.state[i] = Visit::OnStack;
        for &j in &self.inputs[i] {
            self.visit(j)?;
        }
        self.state[i] = Visit::Done;
        self.order.push(i);
        Ok(())
    }
}
