use rusty_asset::animation_graph::{AnimationGraph, GraphNode, ParameterDeclaration};

use super::{AnimatorComponent, AnimatorParameter, LayerState};

impl AnimatorParameter {
    /// The live value a declared parameter starts from when a graph binds: its
    /// declared default; a `Trigger` declares none — it always starts clear.
    fn from_declaration(decl: &ParameterDeclaration) -> Self {
        match decl {
            ParameterDeclaration::Bool(v) => AnimatorParameter::Bool(*v),
            ParameterDeclaration::Float(v) => AnimatorParameter::Float(*v),
            ParameterDeclaration::Int(v) => AnimatorParameter::Int(*v),
            ParameterDeclaration::Trigger => AnimatorParameter::Trigger(false),
        }
    }
}

impl AnimatorComponent {
    pub fn enter_node(&mut self, node: &GraphNode, blend: f32) {
        self.base.enter_node(node, blend);
        self.is_playing = true;
        self.freeze = false;
    }

    pub fn bind_graph(&mut self, graph: &AnimationGraph) {
        for (name, decl) in &graph.parameters {
            if !self.parameters.contains_key(name) {
                self.parameters
                    .insert(name.clone(), AnimatorParameter::from_declaration(decl));
            }
        }
        if let Some(entry) = graph.node(&graph.base.entry) {
            self.enter_node(entry, 0.0);
        }
    }

    pub fn sync_layers(&mut self, graph: &AnimationGraph) {
        self.layers.truncate(graph.layers.len());
        let bound = self.layers.len();
        self.layers
            .extend(graph.layers[bound..].iter().map(|layer| LayerState {
                weight: layer.weight,
                ..LayerState::default()
            }));
        for (state, layer) in self.layers.iter_mut().zip(&graph.layers) {
            if state.name != layer.name {
                state.name.clone_from(&layer.name);
            }
            let machine = &layer.machine;
            let bound = state
                .playback
                .current_node
                .as_deref()
                .is_some_and(|n| machine.node(n).is_some());
            if let (false, Some(entry)) = (bound, machine.node(&machine.entry)) {
                state.playback.enter_node(entry, 0.0);
            }
        }
    }

    pub fn layer_index(&self, name: &str) -> Option<usize> {
        self.layers
            .iter()
            .position(|l| l.name == name)
            .map(|i| i + 1)
    }
}
