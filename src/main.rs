#![allow(dead_code)]

use std::fmt::Display;
use std::sync::Arc;

use eframe::{CreationContext, NativeOptions};
use egui::Label;
use egui_snarl::ui::{PinInfo, SnarlStyle, SnarlViewer};
use egui_snarl::{InPin, NodeId, OutPin, Snarl};
use mlua::Lua;
use serde::{Deserialize, Serialize};

use crate::node::Node;

mod engine;
mod node;
mod script;

#[derive(Serialize, Deserialize)]
struct State {
    snarl: Snarl<Node>,
    snarl_style: SnarlStyle,
}

impl Default for State {
    fn default() -> Self {
        Self {
            snarl: Snarl::new(),
            snarl_style: SnarlStyle::default(),
        }
    }
}

struct App {
    state: State,
    lua: Lua,
}

impl App {
    fn create(cc: &CreationContext) -> Box<dyn eframe::App> {
        let lua = script::init_lua().unwrap_or_else(|e| panic!("{}", e));

        let state = cc
            .storage
            .and_then(|storage| storage.get_string("state"))
            .and_then(|s| ron::from_str(&s).inspect_err(report_error).ok())
            .unwrap_or_default();

        Box::new(App { state, lua })
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("menu").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                if ui.button("Run").clicked() {
                    if let Err(e) = engine::run(&self.lua, &mut self.state.snarl) {
                        report_error(&e);
                    }
                }
            });
        });
        egui::CentralPanel::default().show(ctx, |ui| {
            self.state.snarl.show(
                &mut Viewer,
                &self.state.snarl_style,
                egui::Id::new("editor"),
                ui,
            )
        });
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let state = ron::to_string(&self.state).unwrap_or_else(|e| panic!("{}", e));
        storage.set_string("state", state);
    }
}

struct Viewer;

impl SnarlViewer<Node> for Viewer {
    fn title(&mut self, node: &Node) -> String {
        node.title().to_string()
    }

    fn show_header(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        ui.add(Label::new(snarl[node].title()).selectable(false));
    }

    fn connect(&mut self, from: &OutPin, to: &InPin, snarl: &mut Snarl<Node>) {
        // The pins must have the same type.
        let from_pins = snarl[from.id.node].outputs();
        let to_pins = snarl[to.id.node].inputs();
        if from_pins[from.id.output] != to_pins[to.id.input] {
            return;
        }

        // Only one connection per output pin.
        for &remote in &to.remotes {
            snarl.disconnect(remote, to.id);
        }
        snarl.connect(from.id, to.id);
    }

    fn outputs(&mut self, node: &Node) -> usize {
        node.outputs().len()
    }

    fn inputs(&mut self, node: &Node) -> usize {
        node.inputs().len()
    }

    fn show_input(
        &mut self,
        pin: &InPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) -> PinInfo {
        let node = &snarl[pin.id.node];
        let (name, pin) = &node.inputs().get_index(pin.id.input).unwrap();
        ui.label(name.as_str());
        pin.info()
    }

    fn show_output(
        &mut self,
        pin: &OutPin,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) -> PinInfo {
        let node = &snarl[pin.id.node];
        let (name, pin) = &node.outputs().get_index(pin.id.output).unwrap();
        ui.label(name.as_str());
        pin.info()
    }

    fn has_body(&mut self, node: &Node) -> bool {
        !node.data.is_empty()
    }

    fn show_body(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        ui.vertical(|ui| {
            ui.allocate_space(egui::vec2(100.0, 0.0));
            for (name, control) in &mut snarl[node].data {
                ui.label(name);
                ui.end_row();
                control.update(ui);
            }
        });
    }

    fn has_graph_menu(&mut self, _pos: egui::Pos2, _snarl: &mut Snarl<Node>) -> bool {
        true
    }

    fn show_graph_menu(
        &mut self,
        pos: egui::Pos2,
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        ui.label("New node");
        for (name, entry) in &*script::CATEGORY.lock() {
            entry.menu(name, ui, &mut |prototype| {
                snarl.insert_node(pos, prototype.create());
            });
        }
    }

    fn has_node_menu(&mut self, _node: &Node) -> bool {
        true
    }

    fn show_node_menu(
        &mut self,
        node: NodeId,
        _inputs: &[InPin],
        _outputs: &[OutPin],
        ui: &mut egui::Ui,
        _scale: f32,
        snarl: &mut Snarl<Node>,
    ) {
        if ui.button("Delete").clicked() {
            snarl.remove_node(node);
            ui.close_menu();
        }

        if ui.button("Clone").clicked() {
            let (node, &pos) = snarl.get_node_pos(node).unwrap();
            let pos = pos + egui::vec2(10.0, 10.0);
            snarl.insert_node(pos, node.clone());
            ui.close_menu();
        }
    }
}

pub fn report_error(e: &(impl Display + ?Sized)) {
    rfd::MessageDialog::new()
        .set_title("Error")
        .set_description(format!("{}", e))
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

fn main() -> Result<(), eframe::Error> {
    std::panic::set_hook(Box::new(|pi| {
        report_error(pi);
        std::process::exit(1);
    }));

    let mut options = NativeOptions::default();
    options.viewport.app_id = Some("encode-nexus".to_string());
    options.viewport.icon = Some(Arc::new(
        eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png")).unwrap(),
    ));
    eframe::run_native("EncodeNexus", options, Box::new(App::create))
}
