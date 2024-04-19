use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use egui_snarl::Snarl;
use mlua::prelude::*;
use petgraph::algo::toposort;
use petgraph::graph::DiGraph;
use petgraph::visit::EdgeRef;
use petgraph::Direction;

use crate::node::Node;

pub fn run(lua: &Lua, snarl: &mut Snarl<Node>) -> LuaResult<()> {
    let mut graph = DiGraph::new();
    let mut map = BTreeMap::new();
    for (node, _) in snarl.node_ids() {
        map.insert(node, graph.add_node(node));
    }

    for (out_pin, in_pin) in snarl.wires() {
        graph.add_edge(
            map[&out_pin.node],
            map[&in_pin.node],
            (
                snarl[out_pin.node].output_name(out_pin.output).to_owned(),
                snarl[in_pin.node].input_name(in_pin.input).to_owned(),
            ),
        );
    }

    let Ok(ord) = toposort(&graph, None) else {
        return Err(LuaError::external("cycle detected"));
    };

    let mut inputs = BTreeMap::new();
    for node in ord {
        let input = match inputs.entry(node) {
            Entry::Occupied(input) => input.into_mut(),
            Entry::Vacant(input) => input.insert(lua.create_table()?),
        };

        let snarl_node = &mut snarl[graph[node]];
        for (name, data) in &snarl_node.data {
            input.set(name.as_str(), data.get_value(lua)?)?;
        }

        let output = snarl_node.run(lua, input.clone())?;

        for (name, data) in &mut snarl_node.data {
            let value: LuaValue = output.get(name.as_str())?;
            if !value.is_nil() {
                data.set_value(lua, value)?;
            }
        }

        for edge in graph.edges_directed(node, Direction::Outgoing) {
            let next = edge.target();
            let (out_name, in_name) = edge.weight();

            let input = match inputs.entry(next) {
                Entry::Occupied(input) => input.into_mut(),
                Entry::Vacant(input) => input.insert(lua.create_table()?),
            };
            let out: LuaValue = output.get(out_name.as_str())?;

            input.set(in_name.as_str(), out)?;
        }
    }

    Ok(())
}
