//! Check a recipe and order the nodes its targets need (#1485). Everything that can
//! be wrong with a recipe's wiring is refused here, before any node evaluates, so a
//! bake either runs whole or writes nothing: every node is checked (ids, arity,
//! inputs, links, cycles), not only the ones the targets reach.

use crate::core::collections::Map;

use super::RunError;
use crate::procgen::recipe::{Link, Node, TextureRecipe};

/// The nodes to evaluate and how they connect, as indices into `recipe.nodes`.
pub(super) struct Plan {
    /// Every node a root needs, each after its inputs, in the order a DFS from the
    /// roots (the targets, or under [`Scope::Every`] the sinks) finishes them.
    pub order: Vec<usize>,
    /// Each node's edges: its inputs in the node's order, then the nodes its params
    /// link to (#1494), in [`crate::procgen::recipe::OpKind::links`] order.
    pub inputs: Vec<Vec<usize>>,
    /// How many input slots of planned nodes read each node: its image is dropped
    /// when this many have run.
    pub uses: Vec<usize>,
    /// The requested outputs, in request order; never dropped.
    pub targets: Vec<usize>,
}

/// Which nodes a plan evaluates.
#[derive(Clone, Copy)]
pub(super) enum Scope {
    /// Only the nodes the targets need.
    Needed,
    /// Every node in the recipe, each still dropped after its last consumer (#1486).
    Every,
}

/// Check `recipe` whole and plan the evaluation of `targets`.
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

/// Every node's edges as indices (its inputs, then its links), after checking its
/// arity and that each id exists. A link is an edge like an input: it orders the
/// nodes, counts as a consumer, and closes a cycle like one.
fn wire(recipe: &TextureRecipe, by_id: &Map<&str, usize>) -> Result<Vec<Vec<usize>>, RunError> {
    let wire_node = |node: &Node| {
        node.check_arity().map_err(RunError::Arity)?;
        let index = |input: &String| {
            by_id
                .get(input.as_str())
                .copied()
                .ok_or_else(|| RunError::UnknownInput {
                    node: node.id.clone(),
                    input: input.clone(),
                })
        };
        let link = |(param, link): (&str, &Link)| {
            by_id
                .get(link.node.as_str())
                .copied()
                .ok_or_else(|| RunError::UnknownLink {
                    node: node.id.clone(),
                    param: param.to_string(),
                    link: link.node.clone(),
                })
        };
        let inputs = node.inputs.iter().map(index);
        inputs
            .chain(node.op.links().into_iter().map(link))
            .collect()
    };
    recipe.nodes.iter().map(wire_node).collect()
}

/// The nodes nothing reads, in node-list order. In a DAG every node reaches one, so
/// a DFS from them visits the whole recipe.
fn sinks(inputs: &[Vec<usize>]) -> Vec<usize> {
    let mut read = vec![false; inputs.len()];
    for &j in inputs.iter().flatten() {
        read[j] = true;
    }
    (0..inputs.len()).filter(|&i| !read[i]).collect()
}

/// DFS visit state for cycle detection.
#[derive(Clone, Copy, PartialEq)]
enum Visit {
    Unseen,
    OnStack,
    Done,
}

/// A depth-first topological sort from the given roots, inputs first.
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
            Visit::OnStack => return Err(RunError::Cycle(self.recipe.nodes[i].id.clone())),
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
